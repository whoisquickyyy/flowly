use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use crate::models::{
    AuthStatus, Playlist, ProviderCapabilities, ProxyConfig, SourceKind, Track,
};
use crate::network::proxy::build_client;
use crate::providers::{MusicProvider, ProviderError};

pub struct SoundCloudProvider {
    proxy: ProxyConfig,
    client: Client,
    oauth_token: Option<String>,
    client_id: Option<String>,
    account_name: Option<String>,
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
        }
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
            let resp = self
                .client
                .get("https://api-v2.soundcloud.com/me")
                .header("Authorization", format!("OAuth {}", tok))
                .send()
                .await?
                .error_for_status()?;
            let json: Value = resp.json().await?;
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
        let id = t["id"].as_u64().map(|n| n.to_string())?;
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
            // 1. Попытка через v2 /me/track_likes
            let mut req = self
                .client
                .get("https://api-v2.soundcloud.com/me/track_likes")
                .query(&[("limit", "50")])
                .header("Authorization", format!("OAuth {}", tok));
            if let Some(ref cid) = self.client_id {
                req = req.query(&[("client_id", cid.as_str())]);
            }
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
        let mut base = self.fetch_liked_tracks().await.unwrap_or_default();
        if let Some(first_id) = base.first().map(|t| t.id.trim_start_matches("sc:").to_string()) {
            let mut req = self
                .client
                .get(format!("https://api-v2.soundcloud.com/tracks/{}/related", first_id))
                .query(&[("limit", "20")]);
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
        Ok(base)
    }

    pub async fn set_track_like(&self, track_id: &str, liked: bool) -> Result<(), ProviderError> {
        let tok = self.oauth_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let raw_id = track_id.strip_prefix("sc:").unwrap_or(track_id);
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
        let mut req = self
            .client
            .get("https://api-v2.soundcloud.com/search/tracks")
            .query(&[("q", query), ("limit", "20")]);

        if let Some(ref cid) = self.client_id {
            req = req.query(&[("client_id", cid.as_str())]);
        }
        if let Some(ref tok) = self.oauth_token {
            req = req.header("Authorization", format!("OAuth {}", tok));
        } else if self.client_id.is_none() {
            return Err(ProviderError::Unauthorized);
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
        let mut req = self
            .client
            .get(format!("https://api-v2.soundcloud.com/tracks/{}", raw_id));

        if let Some(ref cid) = self.client_id {
            req = req.query(&[("client_id", cid.as_str())]);
        }
        if let Some(ref tok) = self.oauth_token {
            req = req.header("Authorization", format!("OAuth {}", tok));
        } else if self.client_id.is_none() {
            return Err(ProviderError::Unauthorized);
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

        let mut stream_req = self.client.get(transcoding_url);
        if let Some(ref cid) = self.client_id {
            stream_req = stream_req.query(&[("client_id", cid.as_str())]);
        }
        if let Some(ref tok) = self.oauth_token {
            stream_req = stream_req.header("Authorization", format!("OAuth {}", tok));
        }

        let stream_json: Value = stream_req.send().await?.error_for_status()?.json().await?;
        Ok(stream_json["url"].as_str().map(String::from))
    }
}
