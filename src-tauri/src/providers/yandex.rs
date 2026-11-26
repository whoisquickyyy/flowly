use async_trait::async_trait;
use md5::{Digest, Md5};
use reqwest::Client;
use serde_json::Value;

use crate::models::{
    ArtistRelease, ArtistYandexInfo, AuthStatus, Playlist, ProviderCapabilities, ProxyConfig,
    SourceKind, Track,
};
use crate::network::proxy::build_client;
use crate::providers::{MusicProvider, ProviderError};

pub struct YandexMusicProvider {
    proxy: ProxyConfig,
    client: Client,
    token: Option<String>,
    account_name: Option<String>,
    uid: Option<u64>,
}

impl YandexMusicProvider {
    pub fn new(proxy: ProxyConfig, token: Option<String>, account_name: Option<String>) -> Self {
        let client = build_client(&proxy).unwrap_or_else(|_| Client::new());
        Self {
            proxy,
            client,
            token,
            account_name,
            uid: None,
        }
    }

    pub fn set_credentials(&mut self, token: Option<String>, account_name: Option<String>, uid: Option<u64>) {
        self.token = token;
        self.account_name = account_name;
        self.uid = uid;
    }

    pub async fn verify_and_fetch_account(&mut self, token: &str) -> Result<(String, u64), ProviderError> {
        let resp = self
            .client
            .get("https://api.music.yandex.net/account/status")
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        let account = &json["result"]["account"];
        let uid = account["uid"]
            .as_u64()
            .ok_or_else(|| ProviderError::Api("Не удалось получить UID Яндекс аккаунта".into()))?;
        let display = account["displayName"]
            .as_str()
            .or_else(|| account["login"].as_str())
            .unwrap_or("Yandex User")
            .to_string();

        self.token = Some(token.to_string());
        self.account_name = Some(display.clone());
        self.uid = Some(uid);
        Ok((display, uid))
    }

    async fn ensure_uid(&mut self) -> Result<u64, ProviderError> {
        if let Some(uid) = self.uid {
            return Ok(uid);
        }
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let (_, uid) = self.verify_and_fetch_account(&token).await?;
        Ok(uid)
    }

    fn parse_track(t: &Value) -> Option<Track> {
        let id = t["id"].as_u64().map(|n| n.to_string()).or_else(|| t["id"].as_str().map(String::from))?;
        let title = t["title"].as_str()?.to_string();
        let artist = t["artists"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album_obj = t["albums"].as_array().and_then(|a| a.first());
        let album = album_obj
            .and_then(|a| a["title"].as_str())
            .unwrap_or("Single")
            .to_string();
        let genre = album_obj.and_then(|a| a["genre"].as_str()).map(String::from);
        let release_year = album_obj.and_then(|a| a["year"].as_u64()).map(|y| y as u16);

        let cover_uri = t["coverUri"]
            .as_str()
            .or_else(|| album_obj.and_then(|a| a["coverUri"].as_str()))
            .unwrap_or("");
        let artwork = if cover_uri.is_empty() {
            "https://images.unsplash.com/photo-1614149162883-504ce4d13909?w=600&q=80".to_string()
        } else {
            format!("https://{}", cover_uri.replace("%%", "600x600"))
        };

        let duration = (t["durationMs"].as_u64().unwrap_or(0) / 1000) as u32;
        let external_url = format!("https://music.yandex.ru/track/{}", id);

        Some(Track {
            id: format!("ym:{}", id),
            title,
            artist,
            album,
            artwork,
            duration,
            source: SourceKind::Yandex,
            genre,
            release_year,
            playback_url: None,
            external_url,
            audio_quality: Some("MP3 320 kbps".to_string()),
            rating: None,
            accent_color: "#f59e0b".to_string(),
        })
    }

    pub async fn fetch_liked_tracks(&mut self) -> Result<Vec<Track>, ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let uid = self.ensure_uid().await?;

        let likes_resp = self
            .client
            .get(format!("https://api.music.yandex.net/users/{}/likes/tracks", uid))
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?
            .error_for_status()?;

        let likes_json: Value = likes_resp.json().await?;
        let track_ids: Vec<String> = likes_json["result"]["library"]["tracks"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item["id"].as_str().map(String::from).or_else(|| item["id"].as_u64().map(|n| n.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        if track_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Загружаем всю коллекцию чанками по 100 с ограничением параллелизма (до 6 запросов одновременно)
        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(6));
        let mut set = tokio::task::JoinSet::new();
        for (idx, chunk) in track_ids.chunks(100).enumerate() {
            let client = self.client.clone();
            let tok = token.clone();
            let sem = sem.clone();
            let form_body = format!("track-ids={}", chunk.join(","));
            set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                let resp = client
                    .post("https://api.music.yandex.net/tracks")
                    .header("Authorization", format!("OAuth {}", tok))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(form_body)
                    .send()
                    .await
                    .ok()?
                    .error_for_status()
                    .ok()?;
                let json: Value = resp.json().await.ok()?;
                let tracks: Vec<Track> = json["result"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(Self::parse_track).collect())
                    .unwrap_or_default();
                Some((idx, tracks))
            });
        }

        let mut batches = Vec::new();
        while let Some(res) = set.join_next().await {
            if let Ok(Some(batch)) = res {
                batches.push(batch);
            }
        }
        batches.sort_by_key(|(idx, _)| *idx);
        let all_tracks = batches.into_iter().flat_map(|(_, t)| t).collect();
        Ok(all_tracks)
    }

    pub async fn fetch_liked_albums(&mut self) -> Result<Vec<Track>, ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let uid = self.ensure_uid().await?;

        let resp = self
            .client
            .get(format!("https://api.music.yandex.net/users/{}/likes/albums", uid))
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        let album_ids: Vec<String> = json["result"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        item["id"]
                            .as_u64()
                            .map(|n| n.to_string())
                            .or_else(|| item["id"].as_str().map(String::from))
                    })
                    .take(18)
                    .collect()
            })
            .unwrap_or_default();

        if album_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut set = tokio::task::JoinSet::new();
        for (idx, aid) in album_ids.into_iter().enumerate() {
            let client = self.client.clone();
            let tok = token.clone();
            set.spawn(async move {
                let url = format!("https://api.music.yandex.net/albums/{}/with-tracks", aid);
                let r = client
                    .get(&url)
                    .header("Authorization", format!("OAuth {}", tok))
                    .header("User-Agent", "Yandex-Music-API")
                    .send()
                    .await
                    .ok()?
                    .error_for_status()
                    .ok()?;
                let a_json: Value = r.json().await.ok()?;
                let mut tracks = Vec::new();
                if let Some(volumes) = a_json["result"]["volumes"].as_array() {
                    for vol in volumes {
                        if let Some(arr) = vol.as_array() {
                            for t in arr {
                                if let Some(parsed) = Self::parse_track(t) {
                                    tracks.push(parsed);
                                }
                            }
                        }
                    }
                }
                Some((idx, tracks))
            });
        }

        let mut batches = Vec::new();
        while let Some(res) = set.join_next().await {
            if let Ok(Some(batch)) = res {
                batches.push(batch);
            }
        }
        batches.sort_by_key(|(idx, _)| *idx);
        Ok(batches.into_iter().flat_map(|(_, t)| t).collect())
    }

    pub async fn fetch_wave_tracks_excluding(
        &mut self,
        exclude_ids: &std::collections::HashSet<String>,
        seed_track_id: Option<&str>,
    ) -> Result<Vec<Track>, ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let mut collected: Vec<Track> = Vec::new();
        let mut seen = exclude_ids.clone();
        let mut queue_seed: Option<String> = seed_track_id
            .map(|s| s.trim_start_matches("ym:").to_string());

        for _ in 0..5 {
            let mut req = self
                .client
                .get("https://api.music.yandex.net/rotor/station/user:onyourwave/tracks")
                .query(&[("settings2", "true")])
                .header("Authorization", format!("OAuth {}", token))
                .header("User-Agent", "Yandex-Music-API")
                .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621");
            if let Some(ref seed) = queue_seed {
                req = req.query(&[("queue", seed.as_str())]);
            }

            let Ok(r) = req.send().await else { break };
            let Ok(ok_r) = r.error_for_status() else { break };
            let Ok(json) = ok_r.json::<Value>().await else { break };

            let batch: Vec<Track> = json["result"]["sequence"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| Self::parse_track(&item["track"]))
                        .collect()
                })
                .unwrap_or_default();

            if batch.is_empty() {
                break;
            }
            if let Some(last) = batch.last() {
                queue_seed = Some(last.id.trim_start_matches("ym:").to_string());
            }
            for t in batch {
                if seen.insert(t.id.clone()) {
                    collected.push(t);
                }
            }
            if collected.len() >= 18 {
                break;
            }
        }

        if !collected.is_empty() {
            return Ok(collected);
        }

        // Fallback на любимые треки (исключая уже виденные)
        let mut liked = self.fetch_liked_tracks().await.unwrap_or_default();
        liked.retain(|t| !exclude_ids.contains(&t.id));
        Ok(liked)
    }

    #[allow(dead_code)]
    pub async fn fetch_wave_tracks(&mut self) -> Result<Vec<Track>, ProviderError> {
        let empty = std::collections::HashSet::new();
        self.fetch_wave_tracks_excluding(&empty, None).await
    }

    pub async fn set_track_dislike(&mut self, track_id: &str) -> Result<(), ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let uid = self.ensure_uid().await?;
        let raw_id = track_id.strip_prefix("ym:").unwrap_or(track_id);
        let url = format!("https://api.music.yandex.net/users/{}/dislikes/tracks/add", uid);
        let _ = self
            .client
            .post(&url)
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .form(&[("track-ids", raw_id)])
            .send()
            .await;

        // Также отправим фидбек ротору волны, чтобы этот трек больше не подбирался
        let fb_url = "https://api.music.yandex.net/rotor/station/user:onyourwave/feedback";
        let _ = self
            .client
            .post(fb_url)
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .json(&serde_json::json!({
                "type": "dislike",
                "trackId": raw_id
            }))
            .send()
            .await;
        Ok(())
    }

    pub async fn set_track_like(&mut self, track_id: &str, liked: bool) -> Result<(), ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let uid = self.ensure_uid().await?;
        let raw_id = track_id.strip_prefix("ym:").unwrap_or(track_id);
        let url = if liked {
            format!("https://api.music.yandex.net/users/{}/likes/tracks/add-multiple", uid)
        } else {
            format!("https://api.music.yandex.net/users/{}/likes/tracks/remove", uid)
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .form(&[("track-ids", raw_id)])
            .send()
            .await?;

        if resp.status().is_success() {
            return Ok(());
        }

        // Fallback на /add или query-параметр
        let fallback_url = if liked {
            format!("https://api.music.yandex.net/users/{}/likes/tracks/add", uid)
        } else {
            url
        };
        self.client
            .post(&fallback_url)
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .form(&[("track-ids", raw_id)])
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn fetch_playlists(&mut self) -> Result<Vec<Playlist>, ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let uid = self.ensure_uid().await?;

        let resp = self
            .client
            .get(format!("https://api.music.yandex.net/users/{}/playlists/list", uid))
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        let mut playlists: Vec<Playlist> = json["result"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        let kind = p["kind"].as_u64()?;
                        let title = p["title"].as_str()?.to_string();
                        let cover = p["cover"]["uri"]
                            .as_str()
                            .map(|u| format!("https://{}", u.replace("%%", "400x400")));
                        Some(Playlist {
                            id: format!("ym-pl:{}", kind),
                            title,
                            description: p["description"].as_str().map(String::from),
                            source: Some(SourceKind::Yandex),
                            is_local: false,
                            artwork: cover,
                            tracks: Vec::new(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Подгружаем треки для плейлистов параллельно через /users/{uid}/playlists/{kind}?rich-tracks=true
        // и при необходимости догружаем метаданные треков через POST /tracks
        if !playlists.is_empty() {
            let mut set = tokio::task::JoinSet::new();
            for pl in playlists.iter().take(25) {
                let kind = pl.id.trim_start_matches("ym-pl:").to_string();
                let client = self.client.clone();
                let tok = token.clone();
                set.spawn(async move {
                    let url = format!(
                        "https://api.music.yandex.net/users/{}/playlists/{}",
                        uid, kind
                    );
                    let resp = client
                        .get(&url)
                        .query(&[("rich-tracks", "true")])
                        .header("Authorization", format!("OAuth {}", tok))
                        .header("User-Agent", "Yandex-Music-API")
                        .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
                        .send()
                        .await
                        .ok()?
                        .error_for_status()
                        .ok()?;
                    let json: Value = resp.json().await.ok()?;
                    let t_arr = json["result"]["tracks"].as_array()?;

                    // 1. Пробуем распарсить встроенный объект track
                    let mut tracks: Vec<Track> = t_arr
                        .iter()
                        .filter_map(|item| Self::parse_track(&item["track"]))
                        .collect();

                    // 2. Если Яндекс вернул только id треков без объекта track — запрашиваем POST /tracks
                    if tracks.is_empty() && !t_arr.is_empty() {
                        let ids: Vec<String> = t_arr
                            .iter()
                            .filter_map(|item| {
                                item["id"]
                                    .as_u64()
                                    .map(|n| n.to_string())
                                    .or_else(|| item["id"].as_str().map(String::from))
                                    .or_else(|| item["trackId"].as_u64().map(|n| n.to_string()))
                                    .or_else(|| item["trackId"].as_str().map(String::from))
                            })
                            .take(150)
                            .collect();
                        if !ids.is_empty() {
                            let body = format!("track-ids={}", ids.join(","));
                            if let Ok(tr_resp) = client
                                .post("https://api.music.yandex.net/tracks")
                                .header("Authorization", format!("OAuth {}", tok))
                                .header("User-Agent", "Yandex-Music-API")
                                .header("Content-Type", "application/x-www-form-urlencoded")
                                .body(body)
                                .send()
                                .await
                            {
                                if let Ok(tr_json) = tr_resp.json::<Value>().await {
                                    tracks = tr_json["result"]
                                        .as_array()
                                        .map(|arr| arr.iter().filter_map(Self::parse_track).collect())
                                        .unwrap_or_default();
                                }
                            }
                        }
                    }

                    Some((format!("ym-pl:{}", kind), tracks))
                });
            }

            while let Some(res) = set.join_next().await {
                if let Ok(Some((pl_id, tracks))) = res {
                    if let Some(target) = playlists.iter_mut().find(|pl| pl.id == pl_id) {
                        target.tracks = tracks;
                    }
                }
            }
        }

        Ok(playlists)
    }

    pub async fn fetch_artist_info(
        &self,
        artist_name: &str,
    ) -> Result<Option<ArtistYandexInfo>, ProviderError> {
        let trimmed = artist_name.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let primary = trimmed
            .split(&[',', '&'][..])
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(trimmed);

        let mut candidates = vec![trimmed.to_string()];
        if primary != trimmed {
            candidates.push(primary.to_string());
        }

        let mut found_artist: Option<Value> = None;
        for query in &candidates {
            let mut req = self
                .client
                .get("https://api.music.yandex.net/search")
                .query(&[("text", query.as_str()), ("type", "artist"), ("page", "0")])
                .header("User-Agent", "Yandex-Music-API")
                .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621");
            if let Some(ref token) = self.token {
                req = req.header("Authorization", format!("OAuth {}", token));
            }
            if let Ok(resp) = req.send().await {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        if let Some(arr) = json["result"]["artists"]["results"].as_array() {
                            let ql = query.to_lowercase();
                            let exact = arr.iter().find(|a| {
                                a["name"]
                                    .as_str()
                                    .map(|n| n.to_lowercase() == ql)
                                    .unwrap_or(false)
                            });
                            if let Some(a) = exact.or_else(|| arr.first()) {
                                found_artist = Some(a.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }

        let Some(artist_val) = found_artist else {
            return Ok(None);
        };

        let artist_id = artist_val["id"]
            .as_u64()
            .map(|n| n.to_string())
            .or_else(|| artist_val["id"].as_str().map(String::from));
        let Some(artist_id) = artist_id else {
            return Ok(None);
        };

        let name = artist_val["name"]
            .as_str()
            .unwrap_or(primary)
            .to_string();
        let mut artwork = artist_val["cover"]["uri"]
            .as_str()
            .filter(|u| !u.is_empty())
            .map(|u| format!("https://{}", u.replace("%%", "400x400")));

        let mut req = self
            .client
            .get(format!(
                "https://api.music.yandex.net/artists/{}/brief-info",
                artist_id
            ))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621");
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        let mut monthly_listeners: Option<u64> = None;
        let mut latest_release: Option<ArtistRelease> = None;

        if let Ok(resp) = req.send().await {
            if let Ok(ok_r) = resp.error_for_status() {
                if let Ok(brief) = ok_r.json::<Value>().await {
                    let res = &brief["result"];
                    if artwork.is_none() {
                        artwork = res["artist"]["cover"]["uri"]
                            .as_str()
                            .filter(|u| !u.is_empty())
                            .map(|u| format!("https://{}", u.replace("%%", "400x400")));
                    }
                    monthly_listeners = res["stats"]["lastMonthListeners"]
                        .as_u64()
                        .or_else(|| res["artist"]["stats"]["lastMonthListeners"].as_u64())
                        .or_else(|| res["lastMonthListeners"].as_u64());

                    let rel_val = res["lastReleases"]
                        .as_array()
                        .and_then(|a| a.first())
                        .or_else(|| res["albums"].as_array().and_then(|a| a.first()));

                    if let Some(r) = rel_val {
                        if let Some(rid) = r["id"]
                            .as_u64()
                            .map(|n| n.to_string())
                            .or_else(|| r["id"].as_str().map(String::from))
                        {
                            let r_title = r["title"].as_str().unwrap_or("Релиз").to_string();
                            let r_cover = r["coverUri"]
                                .as_str()
                                .filter(|u| !u.is_empty())
                                .map(|u| format!("https://{}", u.replace("%%", "400x400")))
                                .unwrap_or_else(|| {
                                    artwork.clone().unwrap_or_else(|| {
                                        "https://images.unsplash.com/photo-1614149162883-504ce4d13909?w=400&q=80".to_string()
                                    })
                                });
                            latest_release = Some(ArtistRelease {
                                external_url: format!("https://music.yandex.ru/album/{}", rid),
                                id: rid,
                                title: r_title,
                                artwork: r_cover,
                                year: r["year"].as_u64().map(|y| y as u16),
                                release_type: r["type"].as_str().map(String::from),
                                release_date: r["releaseDate"].as_str().map(String::from),
                                track_count: r["trackCount"].as_u64().map(|c| c as u32),
                            });
                        }
                    }
                }
            }
        }

        // Fallback для слушателей за месяц через web handler, если brief-info не вернул stats
        if monthly_listeners.is_none() {
            if let Ok(resp) = self
                .client
                .get("https://music.yandex.ru/handlers/artist.jsx")
                .query(&[("artist", artist_id.as_str()), ("what", "info"), ("lang", "ru")])
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                )
                .send()
                .await
            {
                if let Ok(json) = resp.json::<Value>().await {
                    monthly_listeners = json["stats"]["lastMonthListeners"]
                        .as_u64()
                        .or_else(|| json["artist"]["stats"]["lastMonthListeners"].as_u64());
                }
            }
        }

        Ok(Some(ArtistYandexInfo {
            external_url: format!("https://music.yandex.ru/artist/{}", artist_id),
            id: artist_id,
            name,
            artwork,
            monthly_listeners,
            latest_release,
        }))
    }

    pub async fn fetch_album_tracks(&self, album_id: &str) -> Result<Vec<Track>, ProviderError> {
        let clean_id = album_id.trim();
        if clean_id.is_empty() {
            return Ok(Vec::new());
        }
        let mut req = self
            .client
            .get(format!(
                "https://api.music.yandex.net/albums/{}/with-tracks",
                clean_id
            ))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621");
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }
        let resp = req.send().await?.error_for_status()?;
        let json: Value = resp.json().await?;
        let mut tracks = Vec::new();
        if let Some(volumes) = json["result"]["volumes"].as_array() {
            for vol in volumes {
                if let Some(arr) = vol.as_array() {
                    for t in arr {
                        if let Some(parsed) = Self::parse_track(t) {
                            tracks.push(parsed);
                        }
                    }
                }
            }
        }
        Ok(tracks)
    }
}

#[async_trait]
impl MusicProvider for YandexMusicProvider {
    fn kind(&self) -> SourceKind {
        SourceKind::Yandex
    }

    fn health_check_url(&self) -> &'static str {
        "https://api.music.yandex.net"
    }

    fn proxy_config(&self) -> &ProxyConfig {
        &self.proxy
    }

    fn set_proxy_config(&mut self, config: ProxyConfig) -> Result<(), ProviderError> {
        self.client = build_client(&config)?;
        self.proxy = config;
        Ok(())
    }

    fn client(&self) -> &Client {
        &self.client
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            can_search: true,
            can_stream_in_app: self.token.is_some(),
            can_remote_control: false,
            can_seek: true,
            can_control_volume: true,
            can_read_library: self.token.is_some(),
            can_read_playlists: self.token.is_some(),
            auth_method: "Yandex OAuth Token".to_string(),
            playback_note: if self.token.is_some() {
                "Прямое воспроизведение через официальный поток Яндекс Музыки".to_string()
            } else {
                "Подключите OAuth-токен для доступа к вашим трекам и воспроизведению".to_string()
            },
        }
    }

    fn auth_status(&self) -> AuthStatus {
        AuthStatus {
            source: SourceKind::Yandex,
            connected: self.token.is_some(),
            account_name: self.account_name.clone(),
            capabilities: self.capabilities(),
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<Track>, ProviderError> {
        let token = self.token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let resp = self
            .client
            .get("https://api.music.yandex.net/search")
            .query(&[("text", query), ("type", "track"), ("page", "0")])
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        Ok(json["result"]["tracks"]["results"]
            .as_array()
            .map(|arr| arr.iter().filter_map(Self::parse_track).collect())
            .unwrap_or_default())
    }

    async fn resolve_playback_url(&self, track_id: &str) -> Result<Option<String>, ProviderError> {
        let token = self.token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let raw_id = track_id.strip_prefix("ym:").unwrap_or(track_id);

        let info_resp = self
            .client
            .get(format!("https://api.music.yandex.net/tracks/{}/download-info", raw_id))
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?
            .error_for_status()?;

        let info_json: Value = info_resp.json().await?;
        let best = info_json["result"]
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .filter(|item| item["codec"].as_str() == Some("mp3"))
                    .max_by_key(|item| item["bitrateInKbps"].as_u64().unwrap_or(0))
                    .or_else(|| arr.first())
            })
            .ok_or_else(|| ProviderError::Api("Поток воспроизведения недоступен".into()))?;

        let download_info_url = best["downloadInfoUrl"]
            .as_str()
            .ok_or_else(|| ProviderError::Api("Отсутствует downloadInfoUrl".into()))?;

        let xml = self
            .client
            .get(download_info_url)
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?
            .text()
            .await?;

        let host = extract_xml_tag(&xml, "host").ok_or_else(|| ProviderError::Api("XML host missing".into()))?;
        let path = extract_xml_tag(&xml, "path").ok_or_else(|| ProviderError::Api("XML path missing".into()))?;
        let ts = extract_xml_tag(&xml, "ts").ok_or_else(|| ProviderError::Api("XML ts missing".into()))?;
        let s = extract_xml_tag(&xml, "s").ok_or_else(|| ProviderError::Api("XML s missing".into()))?;

        let sign_src = format!("XGRlBW9FXlekgbPrRHuSiA{}{}", path.trim_start_matches('/'), s);
        let mut hasher = Md5::new();
        hasher.update(sign_src.as_bytes());
        let sign = format!("{:x}", hasher.finalize());

        Ok(Some(format!("https://{}/get-mp3/{}/{}{}", host, sign, ts, path)))
    }
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_string())
}
