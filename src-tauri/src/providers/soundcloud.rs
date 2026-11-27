use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use crate::models::{
    AuthStatus, Playlist, ProviderCapabilities, ProxyConfig, ProxyMode, SourceKind, Track,
};
use crate::network::proxy::build_client;
use crate::providers::{MusicProvider, ProviderError};

pub struct SoundCloudProvider {
    proxy: ProxyConfig,
    client: Client,
    oauth_token: Option<String>,
    client_id: Option<String>,
    account_name: Option<String>,
    auto_client_id: std::sync::Arc<tokio::sync::RwLock<Option<String>>>,
    auto_fail_until: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl SoundCloudProvider {
    pub fn new(
        proxy: ProxyConfig,
        oauth_token: Option<String>,
        client_id: Option<String>,
        account_name: Option<String>,
    ) -> Self {
        let client = build_client(&proxy).unwrap_or_else(|_| Client::new());
        Self {
            proxy,
            client,
            oauth_token,
            client_id,
            account_name,
            auto_client_id: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
            auto_fail_until: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn sc_endpoint(&self, target: &str) -> String {
        if self.proxy.mode != ProxyMode::Direct {
            return target.to_string();
        }
        let worker = option_env!("FLOWLY_SC_OAUTH_URL")
            .map(|s| s.trim().trim_end_matches('/'))
            .filter(|s| !s.is_empty())
            .unwrap_or("https://flowly-sc-oauth.demashein.workers.dev");
        let encoded: String = url::form_urlencoded::byte_serialize(target.as_bytes()).collect();
        format!("{}/sc-proxy?url={}", worker, encoded)
    }

    pub async fn resolve_public_client_id(&self) -> Option<String> {
        self.resolve_public_client_id_inner(false).await
    }

    pub async fn resolve_public_client_id_fresh(&self) -> Option<String> {
        self.resolve_public_client_id_inner(true).await
    }

    async fn resolve_public_client_id_inner(&self, ignore_cooldown: bool) -> Option<String> {
        if let Some(ref cid) = self.client_id {
            return Some(cid.clone());
        }
        if let Some(ref cached) = *self.auto_client_id.read().await {
            return Some(cached.clone());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if !ignore_cooldown && self.auto_fail_until.load(std::sync::atomic::Ordering::Relaxed) > now {
            return None;
        }

        let html = tokio::time::timeout(
            std::time::Duration::from_secs(6),
            async {
                self.client
                    .get(self.sc_endpoint("https://soundcloud.com"))
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
                    .send()
                    .await
                    .ok()?
                    .text()
                    .await
                    .ok()
            },
        )
        .await
        .ok()
        .flatten();

        let Some(html) = html else {
            self.auto_fail_until.store(now + 180, std::sync::atomic::Ordering::Relaxed);
            return None;
        };

        let mut script_urls = Vec::new();
        let mut rest = html.as_str();
        while let Some(pos) = rest.find("https://a-v2.sndcdn.com/assets/") {
            let sub = &rest[pos..];
            if let Some(end) = sub.find('"') {
                let candidate = &sub[..end];
                if candidate.ends_with(".js") {
                    script_urls.push(candidate.to_string());
                }
                rest = &sub[end..];
            } else {
                break;
            }
        }

        for js_url in script_urls.into_iter().rev().take(12) {
            let Ok(Ok(resp)) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                self.client.get(self.sc_endpoint(&js_url)).send(),
            )
            .await
            else {
                continue;
            };
            let Ok(js) = resp.text().await else {
                continue;
            };
            for needle in ["client_id:\"", "client_id=\"", "client_id:'", "client_id:'"] {
                if let Some(idx) = js.find(needle) {
                    let after = &js[idx + needle.len()..];
                    let quote = if needle.ends_with('\'') { '\'' } else { '"' };
                    if let Some(q) = after.find(quote) {
                        let cid = after[..q].trim();
                        if cid.len() >= 20 && cid.len() <= 48 && cid.chars().all(|c| c.is_ascii_alphanumeric()) {
                            *self.auto_client_id.write().await = Some(cid.to_string());
                            return Some(cid.to_string());
                        }
                    }
                }
            }
        }

        self.auto_fail_until.store(now + 180, std::sync::atomic::Ordering::Relaxed);
        None
    }

    pub fn set_credentials(
        &mut self,
        oauth_token: Option<String>,
        client_id: Option<String>,
        account_name: Option<String>,
    ) {
        self.oauth_token = oauth_token;
        self.client_id = client_id;
        self.account_name = account_name;
    }

    pub async fn verify_credentials(
        &mut self,
        oauth_token: Option<String>,
        client_id: Option<String>,
    ) -> Result<String, ProviderError> {
        if let Some(ref tok) = oauth_token {
            // Сначала проверяем официальный OAuth 2.1 эндпоинт (Bearer), затем fallback на api-v2 (OAuth)
            let mut me_json: Option<Value> = None;
            let mut last_err: Option<ProviderError> = None;

            for (url, auth_val) in [
                ("https://api.soundcloud.com/me", format!("Bearer {}", tok)),
                ("https://api.soundcloud.com/me", format!("OAuth {}", tok)),
                ("https://api-v2.soundcloud.com/me", format!("OAuth {}", tok)),
                ("https://api-v2.soundcloud.com/me", format!("Bearer {}", tok)),
            ] {
                match self
                    .client
                    .get(self.sc_endpoint(url))
                    .header("Authorization", auth_val)
                    .header("Accept", "application/json")
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
                    .send()
                    .await
                {
                    Ok(resp) => match resp.error_for_status() {
                        Ok(ok_r) => match ok_r.json::<Value>().await {
                            Ok(j) => {
                                me_json = Some(j);
                                break;
                            }
                            Err(e) => last_err = Some(e.into()),
                        },
                        Err(e) => last_err = Some(e.into()),
                    },
                    Err(e) => last_err = Some(e.into()),
                }
            }

            let json = match me_json {
                Some(j) => j,
                None => return Err(last_err.unwrap_or(ProviderError::Unauthorized)),
            };

            let name = json["username"]
                .as_str()
                .unwrap_or("SoundCloud User")
                .to_string();
            self.oauth_token = oauth_token;
            self.client_id = client_id;
            self.account_name = Some(name.clone());
            return Ok(name);
        }

        if let Some(ref cid) = client_id {
            self.client
                .get("https://api-v2.soundcloud.com/search/tracks")
                .query(&[("q", "ambient"), ("limit", "1"), ("client_id", cid.as_str())])
                .send()
                .await?
                .error_for_status()?;
            self.client_id = client_id;
            self.account_name = Some("SoundCloud Client".to_string());
            return Ok("SoundCloud Client".to_string());
        }

        Err(ProviderError::Unauthorized)
    }

    fn parse_track(t: &Value) -> Option<Track> {
        let id = t["id"]
            .as_u64()
            .map(|n| n.to_string())
            .or_else(|| t["id"].as_str().map(String::from))?;
        let title = t["title"].as_str()?.to_string();
        let artist = t["user"]["username"]
            .as_str()
            .unwrap_or("SoundCloud Artist")
            .to_string();

        let artwork = t["artwork_url"]
            .as_str()
            .map(|u| u.replace("-large.", "-t500x500."))
            .or_else(|| t["user"]["avatar_url"].as_str().map(String::from))
            .unwrap_or_else(|| {
                "https://images.unsplash.com/photo-1508700115892-45ecd05ae2ad?w=600&q=80"
                    .to_string()
            });

        let duration = (t["duration"].as_u64().unwrap_or(0) / 1000) as u32;
        let genre = t["genre"].as_str().filter(|s| !s.is_empty()).map(String::from);
        let release_year = t["created_at"]
            .as_str()
            .and_then(|d| d.get(0..4))
            .and_then(|y| y.parse::<u16>().ok());

        let external_url = t["permalink_url"]
            .as_str()
            .unwrap_or("https://soundcloud.com")
            .to_string();

        Some(Track {
            id: format!("sc:{}", id),
            title,
            artist,
            album: "SoundCloud".to_string(),
            artwork,
            duration,
            source: SourceKind::SoundCloud,
            genre,
            release_year,
            playback_url: None,
            external_url,
            audio_quality: Some("MP3 128-256 kbps".to_string()),
            rating: None,
            accent_color: "#f97316".to_string(),
        })
    }

    pub async fn fetch_liked_tracks(&self) -> Result<Vec<Track>, ProviderError> {
        if let Some(ref tok) = self.oauth_token {
            // 1. Официальный OAuth 2.1 эндпоинт /me/likes/tracks и /me/favorites (Bearer)
            for url in [
                "https://api.soundcloud.com/me/likes/tracks?limit=50",
                "https://api.soundcloud.com/me/favorites?limit=50",
            ] {
                if let Ok(resp) = self
                    .client
                    .get(self.sc_endpoint(url))
                    .header("Authorization", format!("Bearer {}", tok))
                    .header("Accept", "application/json")
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
                    .send()
                    .await
                {
                    if let Ok(ok_r) = resp.error_for_status() {
                        if let Ok(json) = ok_r.json::<Value>().await {
                            let arr_opt = json.as_array().or_else(|| json["collection"].as_array());
                            if let Some(arr) = arr_opt {
                                let tracks: Vec<Track> = arr
                                    .iter()
                                    .filter_map(|item| {
                                        if item["track"].is_object() {
                                            Self::parse_track(&item["track"])
                                        } else {
                                            Self::parse_track(item)
                                        }
                                    })
                                    .collect();
                                if !tracks.is_empty() {
                                    return Ok(tracks);
                                }
                            }
                        }
                    }
                }
            }

            // 2. Попытка через v2 /me/track_likes (для cookie/OAuth токенов)
            let v2_likes_url = if let Some(ref cid) = self.client_id {
                format!("https://api-v2.soundcloud.com/me/track_likes?limit=50&client_id={}", cid)
            } else {
                "https://api-v2.soundcloud.com/me/track_likes?limit=50".to_string()
            };
            let req = self
                .client
                .get(self.sc_endpoint(&v2_likes_url))
                .header("Authorization", format!("OAuth {}", tok));
            if let Ok(resp) = req.send().await {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        let tracks: Vec<Track> = json["collection"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|item| {
                                        if item["track"].is_object() {
                                            Self::parse_track(&item["track"])
                                        } else {
                                            Self::parse_track(item)
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if !tracks.is_empty() {
                            return Ok(tracks);
                        }
                    }
                }
            }

            // 2. Fallback через v1 /me/favorites (работает только с OAuth токеном без client_id)
            if let Ok(resp) = self
                .client
                .get("https://api.soundcloud.com/me/favorites")
                .query(&[("limit", "50")])
                .header("Authorization", format!("OAuth {}", tok))
                .send()
                .await
            {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        let arr_opt = json.as_array().or_else(|| json["collection"].as_array());
                        if let Some(arr) = arr_opt {
                            let tracks: Vec<Track> = arr.iter().filter_map(Self::parse_track).collect();
                            if !tracks.is_empty() {
                                return Ok(tracks);
                            }
                        }
                    }
                }
            }

            // 3. Fallback через v2 /me/play-history/tracks (работает только с OAuth токеном)
            if let Ok(resp) = self
                .client
                .get("https://api-v2.soundcloud.com/me/play-history/tracks")
                .query(&[("limit", "40")])
                .header("Authorization", format!("OAuth {}", tok))
                .send()
                .await
            {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        let tracks: Vec<Track> = json["collection"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|item| Self::parse_track(&item["track"]))
                                    .collect()
                            })
                            .unwrap_or_default();
                        if !tracks.is_empty() {
                            return Ok(tracks);
                        }
                    }
                }
            }
        }

        if let Some(ref cid) = self.client_id {
            let resp = self
                .client
                .get("https://api-v2.soundcloud.com/charts")
                .query(&[
                    ("kind", "top"),
                    ("genre", "soundcloud:genres:all-music"),
                    ("limit", "30"),
                    ("client_id", cid.as_str()),
                ])
                .send()
                .await?
                .error_for_status()?;
            let json: Value = resp.json().await?;
            return Ok(json["collection"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| Self::parse_track(&item["track"]))
                        .collect()
                })
                .unwrap_or_default());
        }

        Err(ProviderError::Unauthorized)
    }

    pub async fn fetch_wave_tracks(&self) -> Result<Vec<Track>, ProviderError> {
        use rand::seq::SliceRandom;

        let mut base = self.fetch_liked_tracks().await.unwrap_or_default();
        if let Ok(recent) = self.fetch_recently_played().await {
            for t in recent {
                if !base.iter().any(|x| x.id == t.id) {
                    base.push(t);
                }
            }
        }
        {
            let mut rng = rand::thread_rng();
            base.shuffle(&mut rng);
        }

        let seed_ids: Vec<String> = base
            .iter()
            .take(3)
            .map(|t| t.id.trim_start_matches("sc:").to_string())
            .collect();

        for seed_id in seed_ids {
            let mut req = self
                .client
                .get(format!("https://api-v2.soundcloud.com/tracks/{}/related", seed_id))
                .query(&[("limit", "25")]);
            if let Some(ref cid) = self.client_id {
                req = req.query(&[("client_id", cid.as_str())]);
            }
            if let Some(ref tok) = self.oauth_token {
                req = req.header("Authorization", format!("OAuth {}", tok));
            }
            if let Ok(resp) = req.send().await {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        if let Some(arr) = json["collection"].as_array() {
                            let related: Vec<Track> = arr.iter().filter_map(Self::parse_track).collect();
                            for t in related {
                                if !base.iter().any(|x| x.id == t.id) {
                                    base.push(t);
                                }
                            }
                        }
                    }
                }
            }
        }
        {
            let mut rng = rand::thread_rng();
            base.shuffle(&mut rng);
        }
        Ok(base)
    }

    pub async fn set_track_like(&self, track_id: &str, liked: bool) -> Result<(), ProviderError> {
        let tok = self.oauth_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let raw_id = track_id.strip_prefix("sc:").unwrap_or(track_id);
        let v1_like_url = format!("https://api.soundcloud.com/likes/tracks/{}", raw_id);
        let req = if liked {
            self.client.post(&v1_like_url)
        } else {
            self.client.delete(&v1_like_url)
        };
        if let Ok(resp) = req
            .header("Authorization", format!("Bearer {}", tok))
            .header("Accept", "application/json")
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok(());
            }
        }

        let url = format!("https://api.soundcloud.com/me/favorites/{}", raw_id);
        let req = if liked {
            self.client.put(&url)
        } else {
            self.client.delete(&url)
        };
        req.header("Authorization", format!("OAuth {}", tok))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn fetch_recently_played(&self) -> Result<Vec<Track>, ProviderError> {
        if let Some(ref tok) = self.oauth_token {
            let mut req = self
                .client
                .get("https://api-v2.soundcloud.com/me/play-history/tracks")
                .query(&[("limit", "30")])
                .header("Authorization", format!("OAuth {}", tok));
            if let Some(ref cid) = self.client_id {
                req = req.query(&[("client_id", cid.as_str())]);
            }
            if let Ok(resp) = req.send().await {
                if let Ok(ok_resp) = resp.error_for_status() {
                    let json: Value = ok_resp.json().await?;
                    let tracks: Vec<Track> = json["collection"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|item| Self::parse_track(&item["track"]))
                                .collect()
                        })
                        .unwrap_or_default();
                    if !tracks.is_empty() {
                        return Ok(tracks);
                    }
                }
            }
        }
        self.fetch_liked_tracks().await
    }

    pub async fn fetch_playlists(&self) -> Result<Vec<Playlist>, ProviderError> {
        let tok = self.oauth_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        // 1. Официальный OAuth 2.1 эндпоинт /me/playlists (Bearer)
        if let Ok(resp) = self
            .client
            .get(self.sc_endpoint("https://api.soundcloud.com/me/playlists?limit=25"))
            .header("Authorization", format!("Bearer {}", tok))
            .header("Accept", "application/json")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
            .send()
            .await
        {
            if let Ok(ok_r) = resp.error_for_status() {
                if let Ok(json) = ok_r.json::<Value>().await {
                    let arr_opt = json.as_array().or_else(|| json["collection"].as_array());
                    if let Some(arr) = arr_opt {
                        let pls: Vec<Playlist> = arr
                            .iter()
                            .filter_map(|p| {
                                let id = p["id"].as_u64()?;
                                let title = p["title"].as_str()?.to_string();
                                let artwork = p["artwork_url"]
                                    .as_str()
                                    .map(|u| u.replace("-large.", "-t500x500."));
                                let tracks = p["tracks"]
                                    .as_array()
                                    .map(|ts| ts.iter().filter_map(Self::parse_track).collect())
                                    .unwrap_or_default();
                                Some(Playlist {
                                    id: format!("sc-pl:{}", id),
                                    title,
                                    description: p["description"].as_str().map(String::from),
                                    source: Some(SourceKind::SoundCloud),
                                    is_local: false,
                                    artwork,
                                    tracks,
                                })
                            })
                            .collect();
                        if !pls.is_empty() {
                            return Ok(pls);
                        }
                    }
                }
            }
        }

        let mut req = self
            .client
            .get("https://api-v2.soundcloud.com/me/playlists")
            .query(&[("limit", "25")])
            .header("Authorization", format!("OAuth {}", tok));
        if let Some(ref cid) = self.client_id {
            req = req.query(&[("client_id", cid.as_str())]);
        }
        let resp = req.send().await?.error_for_status()?;
        let json: Value = resp.json().await?;
        Ok(json["collection"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        let id = p["id"].as_u64()?;
                        let title = p["title"].as_str()?.to_string();
                        let artwork = p["artwork_url"]
                            .as_str()
                            .map(|u| u.replace("-large.", "-t500x500."));
                        let tracks = p["tracks"]
                            .as_array()
                            .map(|ts| ts.iter().filter_map(Self::parse_track).collect())
                            .unwrap_or_default();
                        Some(Playlist {
                            id: format!("sc-pl:{}", id),
                            title,
                            description: p["description"].as_str().map(String::from),
                            source: Some(SourceKind::SoundCloud),
                            is_local: false,
                            artwork,
                            tracks,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}

#[async_trait]
impl MusicProvider for SoundCloudProvider {
    fn kind(&self) -> SourceKind {
        SourceKind::SoundCloud
    }

    fn health_check_url(&self) -> &'static str {
        "https://soundcloud.com"
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
        let connected = self.oauth_token.is_some() || self.client_id.is_some();
        ProviderCapabilities {
            can_search: connected,
            can_stream_in_app: connected,
            can_remote_control: false,
            can_seek: true,
            can_control_volume: true,
            can_read_library: connected,
            can_read_playlists: self.oauth_token.is_some(),
            auth_method: "OAuth Token / Client ID".to_string(),
            playback_note: if connected {
                "Потоковое воспроизведение официальных progressive MP3-стримов SoundCloud".to_string()
            } else {
                "Укажите OAuth Token или Client ID SoundCloud в настройках".to_string()
            },
        }
    }

    fn auth_status(&self) -> AuthStatus {
        let connected = self.oauth_token.is_some() || self.client_id.is_some();
        AuthStatus {
            source: SourceKind::SoundCloud,
            connected,
            account_name: self.account_name.clone(),
            capabilities: self.capabilities(),
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<Track>, ProviderError> {
        let v1_search = format!(
            "https://api.soundcloud.com/tracks?q={}&limit=20",
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
        );
        if let Some(ref tok) = self.oauth_token {
            if let Ok(resp) = self
                .client
                .get(self.sc_endpoint(&v1_search))
                .header("Authorization", format!("Bearer {}", tok))
                .header("Accept", "application/json")
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
                .send()
                .await
            {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(json) = ok_r.json::<Value>().await {
                        let arr_opt = json.as_array().or_else(|| json["collection"].as_array());
                        if let Some(arr) = arr_opt {
                            let tracks: Vec<Track> = arr.iter().filter_map(Self::parse_track).collect();
                            if !tracks.is_empty() {
                                return Ok(tracks);
                            }
                        }
                    }
                }
            }
        }

        let eff_cid = self.resolve_public_client_id().await;
        let v2_search = if let Some(ref cid) = eff_cid {
            format!(
                "https://api-v2.soundcloud.com/search/tracks?q={}&limit=20&client_id={}",
                url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>(),
                cid
            )
        } else {
            format!(
                "https://api-v2.soundcloud.com/search/tracks?q={}&limit=20",
                url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
            )
        };
        let mut req = self.client.get(self.sc_endpoint(&v2_search));

        if eff_cid.is_none() {
            if let Some(ref tok) = self.oauth_token {
                req = req.header("Authorization", format!("OAuth {}", tok));
            } else {
                return Err(ProviderError::Unauthorized);
            }
        }

        let resp = req.send().await?.error_for_status()?;
        let json: Value = resp.json().await?;
        Ok(json["collection"]
            .as_array()
            .map(|arr| arr.iter().filter_map(Self::parse_track).collect())
            .unwrap_or_default())
    }

    async fn resolve_playback_url(&self, track_id: &str) -> Result<Option<String>, ProviderError> {
        let raw_id = track_id.strip_prefix("sc:").unwrap_or(track_id);
        if let Some(ref tok) = self.oauth_token {
            let v1_streams = format!("https://api.soundcloud.com/tracks/{}/streams", raw_id);
            if let Ok(resp) = self
                .client
                .get(self.sc_endpoint(&v1_streams))
                .header("Authorization", format!("Bearer {}", tok))
                .header("Accept", "application/json")
                .send()
                .await
            {
                if let Ok(ok_r) = resp.error_for_status() {
                    if let Ok(streams) = ok_r.json::<Value>().await {
                        if let Some(u) = streams["http_mp3_128_url"]
                            .as_str()
                            .or_else(|| streams["hls_mp3_128_url"].as_str())
                            .or_else(|| streams["hls_opus_64_url"].as_str())
                        {
                            return Ok(Some(self.sc_endpoint(u)));
                        }
                    }
                }
            }
        }

        let eff_cid = self.resolve_public_client_id().await;
        let v2_track = if let Some(ref cid) = eff_cid {
            format!("https://api-v2.soundcloud.com/tracks/{}?client_id={}", raw_id, cid)
        } else {
            format!("https://api-v2.soundcloud.com/tracks/{}", raw_id)
        };
        let mut req = self.client.get(self.sc_endpoint(&v2_track));

        if eff_cid.is_none() {
            if let Some(ref tok) = self.oauth_token {
                req = req.header("Authorization", format!("OAuth {}", tok));
            } else {
                return Err(ProviderError::Unauthorized);
            }
        }

        let json: Value = req.send().await?.error_for_status()?.json().await?;
        let transcoding_url = json["media"]["transcodings"]
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|t| t["format"]["protocol"].as_str() == Some("progressive"))
                    .or_else(|| arr.first())
            })
            .and_then(|t| t["url"].as_str())
            .ok_or_else(|| ProviderError::Api("Нет доступного аудиопотока".into()))?;

        let trans_target = if let Some(ref cid) = eff_cid {
            let sep = if transcoding_url.contains('?') { '&' } else { '?' };
            format!("{}{}client_id={}", transcoding_url, sep, cid)
        } else {
            transcoding_url.to_string()
        };
        let mut stream_req = self.client.get(self.sc_endpoint(&trans_target));
        if eff_cid.is_none() {
            if let Some(ref tok) = self.oauth_token {
                stream_req = stream_req.header("Authorization", format!("OAuth {}", tok));
            }
        }

        let stream_json: Value = stream_req.send().await?.error_for_status()?.json().await?;
        Ok(stream_json["url"].as_str().map(|u| self.sc_endpoint(u)))
    }
}
