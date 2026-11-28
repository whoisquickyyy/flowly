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
    liked_cache: Option<(std::time::Instant, Vec<Track>)>,
    /// Живая ротор-сессия волны (radioSessionId): держим между пакетами
    rotor_session: Option<String>,
    /// Предыдущий трек для rotor-queue ([prev, current], как у оригинала)
    rotor_prev: Option<String>,
    /// batchId последнего пакета (для plays)
    rotor_batch_id: Option<String>,
    /// Отложенные фидбеки сессии: уезжают следующим пакетом волны
    rotor_deferred: Vec<serde_json::Value>,
    /// Настройки волны («По характеру»/«Под настроение») из rotor/wave/last, кэш
    rotor_settings: Option<Vec<String>>,
    /// Кэш резолвов playback_url: preload и advance должны получить ОДИН И ТОТ ЖЕ
    /// URL (Яндекс ротирует CDN-хосты — два резолва давали разные ссылки и убивали
    /// предзагрузку). Ограничен, чистится при смене кредов.
    resolve_cache: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl YandexMusicProvider {
    pub fn new(proxy: ProxyConfig, token: Option<String>, account_name: Option<String>) -> Self {
        let client = build_client(&proxy).unwrap_or_else(|_| Client::new());
        Self {
            rotor_session: None,
            rotor_prev: None,
            rotor_batch_id: None,
            rotor_deferred: Vec::new(),
            rotor_settings: None,
            resolve_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            proxy,
            client,
            token,
            account_name,
            uid: None,
            liked_cache: None,
        }
    }

    pub fn set_credentials(&mut self, token: Option<String>, account_name: Option<String>, uid: Option<u64>) {
        self.reset_rotor();
        self.resolve_cache.lock().unwrap().clear();
        self.token = token;
        self.account_name = account_name;
        self.uid = uid;
        self.liked_cache = None;
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
        if let Some((ts, ref cached)) = self.liked_cache {
            if ts.elapsed() < std::time::Duration::from_secs(120) && !cached.is_empty() {
                return Ok(cached.clone());
            }
        }
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
                let res = &a_json["result"];
                let is_podcast = res["type"].as_str() == Some("podcast")
                    || res["metaType"].as_str() == Some("podcast")
                    || res["genre"].as_str() == Some("podcasts");
                if is_podcast {
                    return None;
                }
                let mut tracks = Vec::new();
                if let Some(volumes) = res["volumes"].as_array() {
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
        let result: Vec<Track> = batches.into_iter().flat_map(|(_, t)| t).collect();
        if !result.is_empty() {
            self.liked_cache = Some((std::time::Instant::now(), result.clone()));
        }
        Ok(result)
    }

    // ---------- волна: rotor-сессии ----------

    async fn rotor_post(&self, url: &str, body: &serde_json::Value) -> Result<Value, ProviderError> {
        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .json(body)
            .send()
            .await?;
        Ok(resp.error_for_status()?.json::<Value>().await?)
    }

    fn parse_rotor_batch(v: &Value) -> Option<(String, Vec<Track>)> {
        let batch_id = v["result"]["batchId"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let tracks: Vec<Track> = v["result"]["sequence"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|item| Self::parse_track(&item["track"])).collect())
            .unwrap_or_default();
        if tracks.is_empty() {
            None
        } else {
            Some((batch_id, tracks))
        }
    }

    /// Настройки волны («По характеру»/«Под настроение»/«По языку»), выбранные в
    /// любом клиенте Яндекса: seeds `setting…` из rotor/wave/last, если последняя
    /// волна была «Моей». Кэшируется на жизнь провайдера.
    async fn rotor_settings_seeds(&mut self) -> Vec<String> {
        if let Some(ref s) = self.rotor_settings {
            return s.clone();
        }
        let Some(ref token) = self.token.clone() else {
            return Vec::new();
        };
        let res = self
            .client
            .get("https://api.music.yandex.net/rotor/wave/last")
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .send()
            .await;
        let seeds = match res {
            Ok(r) => match r.json::<Value>().await {
                Ok(v) => {
                    let arr = v["result"]["seeds"].as_array().cloned().unwrap_or_default();
                    let strs: Vec<String> =
                        arr.into_iter().filter_map(|x| x.as_str().map(String::from)).collect();
                    let (settings, rest): (Vec<String>, Vec<String>) =
                        strs.into_iter().partition(|s| s.starts_with("setting"));
                    if rest.len() == 1 && rest[0] == "user:onyourwave" {
                        settings
                    } else {
                        Vec::new()
                    }
                }
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        self.rotor_settings = Some(seeds.clone());
        seeds
    }

    /// Очередной пакет волны внутри rotor-сессии; при отсутствии/поломке сессии
    /// заводит новую (`session/new` c seeds станции + настройками волны).
    /// Возвращает (batchId, треки).
    async fn rotor_session_tracks(
        &mut self,
        queue: Vec<String>,
    ) -> Result<(String, Vec<Track>), ProviderError> {
        if let Some(ref sid) = self.rotor_session.clone() {
            let mut body = serde_json::json!({ "queue": queue, "includeTracksInResponse": true });
            let deferred = std::mem::take(&mut self.rotor_deferred);
            if !deferred.is_empty() {
                body["feedbacks"] = serde_json::json!(deferred);
            }
            match self
                .rotor_post(
                    &format!("https://api.music.yandex.net/rotor/session/{sid}/tracks"),
                    &body,
                )
                .await
            {
                Ok(v) => {
                    if let Some((bid, tracks)) = Self::parse_rotor_batch(&v) {
                        return Ok((bid, tracks));
                    }
                }
                Err(_) => {
                    // Сессия истекла/сломалась — начнём новую ниже
                    self.rotor_session = None;
                }
            }
        }

        let settings = self.rotor_settings_seeds().await;
        let mut seeds = vec!["user:onyourwave".to_string()];
        seeds.extend(settings);
        let mut body = serde_json::json!({
            "seeds": seeds,
            "includeTracksInResponse": true,
            "includeWaveModel": true,
            "interactive": true,
        });
        if !queue.is_empty() {
            body["queue"] = serde_json::json!(queue);
        }
        let v = self
            .rotor_post("https://api.music.yandex.net/rotor/session/new", &body)
            .await?;
        let sid = v["result"]["radioSessionId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        self.rotor_session = Some(sid.clone());
        let (bid, tracks) = Self::parse_rotor_batch(&v).unwrap_or((String::new(), Vec::new()));
        if !sid.is_empty() {
            self.rotor_track_event("radioStarted", None, None).await;
        }
        Ok((bid, tracks))
    }

    /// Фидбек события сессии ротора (radioStarted/trackStarted/trackFinished/skip):
    /// при живой сессии уходит сразу, иначе ждёт следующего пакета волны.
    pub async fn rotor_track_event(
        &mut self,
        kind: &str,
        track_id: Option<&str>,
        played_sec: Option<f64>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut event = serde_json::json!({ "type": kind, "timestamp": now });
        if kind == "radioStarted" {
            event["from"] = serde_json::json!("flowly-wave");
        }
        if let Some(t) = track_id {
            event["trackId"] = serde_json::json!(t.trim_start_matches("ym:"));
        }
        if let Some(p) = played_sec {
            event["totalPlayedSeconds"] = serde_json::json!(p);
        }
        let body = serde_json::json!({ "event": event, "from": "flowly-wave" });
        if let Some(ref sid) = self.rotor_session.clone() {
            let url = format!("https://api.music.yandex.net/rotor/session/{sid}/feedback/");
            let _ = self.rotor_post(&url, &body).await;
        } else {
            self.rotor_deferred.push(body);
        }
    }

    /// Выход из контекста волны (плейлист/станция/медиатека) гасит rotor-сессию:
    /// иначе она протянула бы устаревший контекст до следующего запуска волны.
    /// Как clear_queue() у оригинала. Настройки волны (rotor_settings) сохраняются.
    pub fn reset_rotor(&mut self) {
        self.rotor_session = None;
        self.rotor_prev = None;
        self.rotor_batch_id = None;
        self.rotor_deferred.clear();
    }

    /// Живая rotor-сессия: пока она есть, поток курует сервер, и локальная
    /// пересортировка хвоста очереди по псевдо-BPM его выдачу не трогает.
    pub fn rotor_active(&self) -> bool {
        self.rotor_session.is_some()
    }

    /// plays при смене трека + фидбек сессии: без plays Яндекс не знает, что
    /// звучало, и волна не адаптируется под вкус (как у оригинала).
    pub async fn rotor_report_play(&mut self, track_id: &str, played_sec: f64, finished: bool) {
        let Some(ref token) = self.token.clone() else { return };
        let raw = track_id.trim_start_matches("ym:").to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut play = serde_json::json!({
            "trackId": raw,
            "from": now - played_sec.min(3600.0),
            "to": now,
            "timestamp": now,
            "context": { "type": "radio", "station": "user:onyourwave" },
            "audioOutputType": "Speaker",
            "maxPlayerStage": "play",
            "isFromAutoflow": false,
        });
        if let Some(ref bid) = self.rotor_batch_id {
            play["batchId"] = serde_json::json!(bid);
        }
        if let Some(ref sid) = self.rotor_session {
            play["radioSessionId"] = serde_json::json!(sid);
        }
        let url = format!("https://api.music.yandex.net/plays?clientNow={}", now as i64);
        let _ = self
            .client
            .post(&url)
            .header("Authorization", format!("OAuth {}", token))
            .header("User-Agent", "Yandex-Music-API")
            .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
            .json(&serde_json::json!({ "plays": [play] }))
            .send()
            .await;
        let kind = if finished { "trackFinished" } else { "skip" };
        self.rotor_track_event(kind, Some(&raw), Some(played_sec)).await;
    }

    pub async fn fetch_wave_tracks_excluding(
        &mut self,
        exclude_ids: &std::collections::HashSet<String>,
        seed_track_id: Option<&str>,
    ) -> Result<Vec<Track>, ProviderError> {
        use rand::seq::SliceRandom;

        let token = self.token.clone().ok_or(ProviderError::Unauthorized)?;
        let mut collected: Vec<Track> = Vec::new();
        let mut seen = exclude_ids.clone();
        let current = seed_track_id.map(|s| s.trim_start_matches("ym:").to_string());

        // Ротор-сессия: до 3 пакетов с queue=[prev, current] и отложенными
        // фидбеками (протокол оригинала: сессия живёт, сервер видит скипы/дослушивания)
        for _ in 0..3 {
            let mut queue: Vec<String> = Vec::new();
            if let Some(ref p) = self.rotor_prev {
                queue.push(p.clone());
            }
            if let Some(ref c) = current {
                queue.push(c.clone());
            }
            let Ok((batch_id, batch)) = self.rotor_session_tracks(queue).await else {
                break;
            };
            self.rotor_batch_id = Some(batch_id);
            self.rotor_prev = current.clone();
            let mut last_raw = current.clone();
            for t in &batch {
                let raw = t.id.trim_start_matches("ym:").to_string();
                if seen.insert(t.id.clone()) {
                    collected.push(t.clone());
                }
                last_raw = Some(raw);
            }
            if collected.len() >= 15 || batch.is_empty() {
                break;
            }
            // Следующий пакет: последний трек пакета «заиграл»
            self.rotor_prev = last_raw;
        }

        // Если ротор вернул мало треков — добираем похожие треки и перемешанные лайки
        if collected.len() < 12 {
            let sim_seed = seed_track_id
                .map(|s| s.trim_start_matches("ym:").to_string())
                .or_else(|| collected.first().map(|t| t.id.trim_start_matches("ym:").to_string()));
            if let Some(raw_seed) = sim_seed {
                let sim_url = format!("https://api.music.yandex.net/tracks/{}/similar", raw_seed);
                if let Ok(resp) = self
                    .client
                    .get(&sim_url)
                    .header("Authorization", format!("OAuth {}", token))
                    .header("User-Agent", "Yandex-Music-API")
                    .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621")
                    .send()
                    .await
                {
                    if let Ok(ok_r) = resp.error_for_status() {
                        if let Ok(json) = ok_r.json::<Value>().await {
                            if let Some(arr) = json["result"]["similarTracks"].as_array() {
                                for item in arr {
                                    if let Some(t) = Self::parse_track(item) {
                                        if seen.insert(t.id.clone()) {
                                            collected.push(t);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if collected.len() < 12 {
            let mut liked = self.fetch_liked_tracks().await.unwrap_or_default();
            {
                let mut rng = rand::thread_rng();
                liked.shuffle(&mut rng);
            }
            for t in liked {
                if seen.insert(t.id.clone()) {
                    collected.push(t);
                    if collected.len() >= 24 {
                        break;
                    }
                }
            }
        }

        Ok(collected)
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
            .map(|u| format!("https://{}", u.replace("%%", "1000x1000")));

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
        let mut similar_artists: Vec<crate::models::SimilarArtist> = Vec::new();
        let mut albums: Vec<ArtistRelease> = Vec::new();
        let mut singles: Vec<ArtistRelease> = Vec::new();
        let mut popular_tracks: Vec<Track> = Vec::new();
        let mut genres: Vec<String> = Vec::new();

        if let Ok(resp) = req.send().await {
            if let Ok(ok_r) = resp.error_for_status() {
                if let Ok(brief) = ok_r.json::<Value>().await {
                    let res = &brief["result"];
                    if artwork.is_none() {
                        artwork = res["artist"]["cover"]["uri"]
                            .as_str()
                            .or_else(|| res["artist"]["ogImage"].as_str())
                            .filter(|u| !u.is_empty())
                            .map(|u| format!("https://{}", u.replace("%%", "1000x1000")));
                    }
                    if let Some(g_arr) = res["artist"]["genres"].as_array() {
                        for g in g_arr.iter().take(4) {
                            if let Some(gs) = g.as_str().filter(|s| !s.is_empty()) {
                                genres.push(gs.to_string());
                            }
                        }
                    }
                    monthly_listeners = res["stats"]["lastMonthListeners"]
                        .as_u64()
                        .or_else(|| res["artist"]["stats"]["lastMonthListeners"].as_u64())
                        .or_else(|| res["lastMonthListeners"].as_u64());

                    let parse_release = |r: &Value| -> Option<ArtistRelease> {
                        let rid = r["id"]
                            .as_u64()
                            .map(|n| n.to_string())
                            .or_else(|| r["id"].as_str().map(String::from))?;
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
                        Some(ArtistRelease {
                            external_url: format!("https://music.yandex.ru/album/{}", rid),
                            id: rid,
                            title: r_title,
                            artwork: r_cover,
                            year: r["year"].as_u64().map(|y| y as u16),
                            release_type: r["type"].as_str().map(String::from),
                            release_date: r["releaseDate"].as_str().map(String::from),
                            track_count: r["trackCount"].as_u64().map(|c| c as u32),
                        })
                    };

                    let rel_val = res["lastReleases"]
                        .as_array()
                        .and_then(|a| a.first())
                        .or_else(|| res["albums"].as_array().and_then(|a| a.first()));

                    if let Some(r) = rel_val {
                        latest_release = parse_release(r);
                    }

                    let mut seen_rel_ids = std::collections::HashSet::new();
                    if let Some(alb_arr) = res["albums"].as_array() {
                        for r in alb_arr.iter().take(40) {
                            if let Some(rel) = parse_release(r) {
                                if !seen_rel_ids.insert(rel.id.clone()) {
                                    continue;
                                }
                                let rtype = rel.release_type.as_deref().unwrap_or("").to_lowercase();
                                if rtype == "single" || rtype == "ep" {
                                    if singles.len() < 24 {
                                        singles.push(rel);
                                    }
                                } else if albums.len() < 24 {
                                    albums.push(rel);
                                }
                            }
                        }
                    }
                    if let Some(last_arr) = res["lastReleases"].as_array() {
                        for r in last_arr.iter().take(24) {
                            if let Some(rel) = parse_release(r) {
                                if !seen_rel_ids.insert(rel.id.clone()) {
                                    continue;
                                }
                                let rtype = rel.release_type.as_deref().unwrap_or("").to_lowercase();
                                if rtype == "single" || rtype == "ep" {
                                    if singles.len() < 24 {
                                        singles.push(rel);
                                    }
                                } else if albums.len() < 24 {
                                    albums.push(rel);
                                }
                            }
                        }
                    }
                    if let Some(pop_arr) = res["popularTracks"].as_array() {
                        for t in pop_arr.iter().take(20) {
                            if let Some(tr) = Self::parse_track(t) {
                                popular_tracks.push(tr);
                            }
                        }
                    }

                    if let Some(sim_arr) = res["similarArtists"].as_array() {
                        for sa in sim_arr.iter().take(6) {
                            let sid = sa["id"]
                                .as_u64()
                                .map(|n| n.to_string())
                                .or_else(|| sa["id"].as_str().map(String::from));
                            let Some(sid) = sid else { continue };
                            let sname = sa["name"].as_str().unwrap_or("").trim().to_string();
                            if sname.is_empty() {
                                continue;
                            }
                            let scover = sa["cover"]["uri"]
                                .as_str()
                                .filter(|u| !u.is_empty())
                                .map(|u| format!("https://{}", u.replace("%%", "200x200")));
                            similar_artists.push(crate::models::SimilarArtist {
                                id: sid,
                                name: sname,
                                artwork: scover,
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
            similar_artists,
            albums,
            singles,
            popular_tracks,
            genres,
        }))
    }

    pub async fn fetch_artist_preview_track(
        &self,
        artist_id: &str,
        artist_name: &str,
    ) -> Result<Option<Track>, ProviderError> {
        let mut picked: Option<Track> = None;
        if !artist_id.trim().is_empty() {
            let mut req = self
                .client
                .get(format!(
                    "https://api.music.yandex.net/artists/{}/tracks",
                    artist_id.trim()
                ))
                .query(&[("page", "0"), ("page-size", "3")])
                .header("User-Agent", "Yandex-Music-API")
                .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621");
            if let Some(ref token) = self.token {
                req = req.header("Authorization", format!("OAuth {}", token));
            }
            if let Ok(resp) = req.send().await {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        picked = json["result"]["tracks"]
                            .as_array()
                            .and_then(|arr| arr.iter().find_map(Self::parse_track));
                    }
                }
            }
        }
        if picked.is_none() && !artist_name.trim().is_empty() {
            if let Ok(list) = self.search(artist_name.trim()).await {
                picked = list.into_iter().next();
            }
        }
        if let Some(mut t) = picked {
            if let Ok(Some(url)) = self.resolve_playback_url(&t.id).await {
                t.playback_url = Some(url);
            }
            return Ok(Some(t));
        }
        Ok(None)
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

        // Кэш: preload_next_track и next_track резолвят один трек дважды подряд —
        // без кэша получались разные CDN-хосты и предзагруженная дека перезагружалась
        if let Some(cached) = self.resolve_cache.lock().unwrap().get(raw_id) {
            return Ok(Some(cached.clone()));
        }

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

        let url = format!("https://{}/get-mp3/{}/{}{}", host, sign, ts, path);
        let mut cache = self.resolve_cache.lock().unwrap();
        if cache.len() > 64 {
            cache.clear();
        }
        cache.insert(raw_id.to_string(), url.clone());
        Ok(Some(url))
    }
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_string())
}
