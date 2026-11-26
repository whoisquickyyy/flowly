use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{
    AuthStatus, Playlist, ProviderCapabilities, ProxyConfig, SourceKind, Track,
};
use crate::network::proxy::build_client;
use crate::providers::{MusicProvider, ProviderError};

pub struct SpotifyProvider {
    proxy: ProxyConfig,
    client: Client,
    access_token: Option<String>,
    account_name: Option<String>,
}

impl SpotifyProvider {
    pub fn new(proxy: ProxyConfig, access_token: Option<String>, account_name: Option<String>) -> Self {
        let client = build_client(&proxy).unwrap_or_else(|_| Client::new());
        Self {
            proxy,
            client,
            access_token,
            account_name,
        }
    }

    pub fn set_credentials(&mut self, token: Option<String>, account_name: Option<String>) {
        self.access_token = token;
        self.account_name = account_name;
    }

    pub async fn verify_and_fetch_account(&mut self, token: &str) -> Result<String, ProviderError> {
        let resp = self
            .client
            .get("https://api.spotify.com/v1/me")
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        let display = json["display_name"]
            .as_str()
            .or_else(|| json["id"].as_str())
            .unwrap_or("Spotify User")
            .to_string();

        self.access_token = Some(token.to_string());
        self.account_name = Some(display.clone());
        Ok(display)
    }

    pub async fn refresh_access_token(
        &mut self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<(String, Option<String>), ProviderError> {
        let resp = self
            .client
            .post("https://accounts.spotify.com/api/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
            ])
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        let new_access = json["access_token"]
            .as_str()
            .ok_or_else(|| ProviderError::Api("Не получен новый access_token".into()))?
            .to_string();
        let new_refresh = json["refresh_token"].as_str().map(String::from);
        self.access_token = Some(new_access.clone());
        Ok((new_access, new_refresh))
    }

    fn parse_track(t: &Value) -> Option<Track> {
        let id = t["id"].as_str()?.to_string();
        let title = t["name"].as_str()?.to_string();
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

        let album = t["album"]["name"].as_str().unwrap_or("Single").to_string();
        let release_year = t["album"]["release_date"]
            .as_str()
            .and_then(|d| d.get(0..4))
            .and_then(|y| y.parse::<u16>().ok());

        let artwork = t["album"]["images"]
            .as_array()
            .and_then(|imgs| imgs.first())
            .and_then(|img| img["url"].as_str())
            .unwrap_or("https://images.unsplash.com/photo-1514525253161-7a46d19cd819?w=600&q=80")
            .to_string();

        let duration = (t["duration_ms"].as_u64().unwrap_or(0) / 1000) as u32;
        let playback_url = t["preview_url"].as_str().map(String::from);
        let external_url = t["external_urls"]["spotify"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| format!("https://open.spotify.com/track/{}", id));

        let popularity = t["popularity"].as_f64().map(|p| (p as f32 / 20.0).clamp(0.0, 5.0));

        Some(Track {
            id: format!("sp:{}", id),
            title,
            artist,
            album,
            artwork,
            duration,
            source: SourceKind::Spotify,
            genre: None,
            release_year,
            playback_url,
            external_url,
            audio_quality: Some("Ogg Vorbis 320 kbps (Connect)".to_string()),
            rating: popularity,
            accent_color: "#10b981".to_string(),
        })
    }

    pub async fn fetch_liked_tracks(&self) -> Result<Vec<Track>, ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let mut all = Vec::new();
        for offset in [0, 50] {
            let resp = self
                .client
                .get(format!("https://api.spotify.com/v1/me/tracks?limit=50&offset={}", offset))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?;

            let json: Value = resp.json().await?;
            let batch: Vec<Track> = json["items"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|i| Self::parse_track(&i["track"])).collect())
                .unwrap_or_default();
            let count = batch.len();
            all.extend(batch);
            if count < 50 {
                break;
            }
        }
        Ok(all)
    }

    pub async fn fetch_all_library_tracks(&self) -> Result<Vec<Track>, ProviderError> {
        let mut out = self.fetch_liked_tracks().await.unwrap_or_default();
        let mut seen: std::collections::HashSet<String> =
            out.iter().map(|t| t.id.clone()).collect();

        if let Ok(recent) = self.fetch_recently_played().await {
            for t in recent {
                if seen.insert(t.id.clone()) {
                    out.push(t);
                }
            }
        }

        if out.is_empty() {
            if let Ok(pls) = self.fetch_playlists().await {
                for pl in pls.into_iter().take(2) {
                    for t in pl.tracks {
                        if seen.insert(t.id.clone()) {
                            out.push(t);
                        }
                    }
                }
            }
        }

        Ok(out)
    }

    pub async fn fetch_wave_tracks(&self) -> Result<Vec<Track>, ProviderError> {
        let mut out = self.fetch_liked_tracks().await.unwrap_or_default();
        if let Ok(recent) = self.fetch_recently_played().await {
            for t in recent {
                if !out.iter().any(|x| x.id == t.id) {
                    out.push(t);
                }
            }
        }
        Ok(out)
    }

    pub async fn set_track_like(&self, track_id: &str, liked: bool) -> Result<(), ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let raw_id = track_id.strip_prefix("sp:").unwrap_or(track_id);
        let req = if liked {
            self.client.put("https://api.spotify.com/v1/me/tracks")
        } else {
            self.client.delete("https://api.spotify.com/v1/me/tracks")
        };
        req.bearer_auth(token)
            .json(&json!({ "ids": [raw_id] }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn fetch_recently_played(&self) -> Result<Vec<Track>, ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let resp = self
            .client
            .get("https://api.spotify.com/v1/me/player/recently-played?limit=20")
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        Ok(json["items"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|i| Self::parse_track(&i["track"])).collect())
            .unwrap_or_default())
    }

    pub async fn fetch_playlists(&self) -> Result<Vec<Playlist>, ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let resp = self
            .client
            .get("https://api.spotify.com/v1/me/playlists?limit=25")
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        Ok(json["items"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        let id = p["id"].as_str()?;
                        let title = p["name"].as_str()?.to_string();
                        let artwork = p["images"]
                            .as_array()
                            .and_then(|imgs| imgs.first())
                            .and_then(|img| img["url"].as_str())
                            .map(String::from);
                        Some(Playlist {
                            id: format!("sp-pl:{}", id),
                            title,
                            description: p["description"].as_str().map(String::from),
                            source: Some(SourceKind::Spotify),
                            is_local: false,
                            artwork,
                            tracks: Vec::new(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn connect_play(&self, track_id: &str) -> Result<(), ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let raw_id = track_id.strip_prefix("sp:").unwrap_or(track_id);
        let uri = format!("spotify:track:{}", raw_id);
        self.client
            .put("https://api.spotify.com/v1/me/player/play")
            .bearer_auth(token)
            .json(&json!({ "uris": [uri] }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn connect_command(&self, method: &str, endpoint: &str) -> Result<(), ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let url = format!("https://api.spotify.com/v1/me/player/{}", endpoint);
        let req = match method {
            "POST" => self.client.post(&url),
            _ => self.client.put(&url),
        };
        req.bearer_auth(token).send().await?.error_for_status()?;
        Ok(())
    }
}

#[async_trait]
impl MusicProvider for SpotifyProvider {
    fn kind(&self) -> SourceKind {
        SourceKind::Spotify
    }

    fn health_check_url(&self) -> &'static str {
        "https://api.spotify.com"
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
            can_search: self.access_token.is_some(),
            can_stream_in_app: false,
            can_remote_control: self.access_token.is_some(),
            can_seek: self.access_token.is_some(),
            can_control_volume: self.access_token.is_some(),
            can_read_library: self.access_token.is_some(),
            can_read_playlists: self.access_token.is_some(),
            auth_method: "OAuth 2.0 PKCE".to_string(),
            playback_note: if self.access_token.is_some() {
                "Полное управление через Spotify Connect (требуется активное устройство Spotify) + 30с превью".to_string()
            } else {
                "Войдите через Spotify OAuth PKCE (в РФ требуется прокси)".to_string()
            },
        }
    }

    fn auth_status(&self) -> AuthStatus {
        AuthStatus {
            source: SourceKind::Spotify,
            connected: self.access_token.is_some(),
            account_name: self.account_name.clone(),
            capabilities: self.capabilities(),
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<Track>, ProviderError> {
        let token = self.access_token.as_ref().ok_or(ProviderError::Unauthorized)?;
        let resp = self
            .client
            .get("https://api.spotify.com/v1/search")
            .query(&[("q", query), ("type", "track"), ("limit", "20")])
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;

        let json: Value = resp.json().await?;
        Ok(json["tracks"]["items"]
            .as_array()
            .map(|arr| arr.iter().filter_map(Self::parse_track).collect())
            .unwrap_or_default())
    }

    async fn resolve_playback_url(&self, _track_id: &str) -> Result<Option<String>, ProviderError> {
        // Официальный Spotify API не отдает прямые полные потоки (DRM).
        // Воспроизведение идёт через Spotify Connect или preview_url.
        Ok(None)
    }
}
