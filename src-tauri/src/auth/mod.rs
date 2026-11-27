use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::{AudioEffectsConfig, Playlist, ProxyConfig, RepeatMode, SourceKind, Track};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredSecrets {
    pub yandex_token: Option<String>,
    pub yandex_account: Option<String>,
    pub soundcloud_token: Option<String>,
    pub soundcloud_client_id: Option<String>,
    pub soundcloud_account: Option<String>,
    #[serde(default)]
    pub soundcloud_session_id: Option<String>,
    #[serde(default)]
    pub soundcloud_oauth_relay: Option<String>,
    pub proxies: HashMap<SourceKind, ProxyConfig>,
    pub local_playlists: Vec<Playlist>,
    #[serde(default)]
    pub effects: Option<AudioEffectsConfig>,
    #[serde(default)]
    pub liked_ids: HashSet<String>,
    #[serde(default)]
    pub disliked_ids: HashSet<String>,
    #[serde(default)]
    pub liked_tracks: Vec<Track>,
    #[serde(default)]
    pub last_track: Option<Track>,
    #[serde(default)]
    pub last_queue: Vec<Track>,
    #[serde(default)]
    pub volume: Option<u8>,
    #[serde(default)]
    pub shuffle: Option<bool>,
    #[serde(default)]
    pub shuffle_mode: Option<crate::models::ShuffleMode>,
    #[serde(default)]
    pub repeat: Option<RepeatMode>,
    #[serde(default)]
    pub doh_enabled: Option<bool>,
    #[serde(default)]
    pub doh_provider: Option<String>,
    #[serde(default)]
    pub spicy_lyrics_key: Option<String>,
    #[serde(default)]
    pub discord_rpc_enabled: Option<bool>,
    #[serde(default)]
    pub discord_client_id: Option<String>,
    #[serde(default)]
    pub close_to_tray: Option<bool>,
    #[serde(default)]
    pub dynamic_theme_enabled: Option<bool>,
    #[serde(default)]
    pub weather_city: Option<String>,
    #[serde(default)]
    pub local_folders: Vec<String>,
}

/// Состояние OAuth-сессии SoundCloud в Rust (единственный источник правды).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundCloudOAuthStatus {
    pub session_id: Option<String>,
    pub pending: bool,
    pub authenticated: bool,
    pub account_name: Option<String>,
    pub auth_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SoundCloudOAuthState {
    pub session_id: Option<String>,
    pub code_verifier: Option<String>,
    pub relay_url: Option<String>,
    pub pending: bool,
    pub authenticated: bool,
    pub account_name: Option<String>,
    pub auth_url: Option<String>,
    pub error: Option<String>,
}

impl SoundCloudOAuthState {
    pub fn from_secrets(secrets: &StoredSecrets) -> Self {
        let authed = secrets.soundcloud_token.is_some() || secrets.soundcloud_client_id.is_some();
        Self {
            session_id: secrets.soundcloud_session_id.clone(),
            code_verifier: None,
            relay_url: secrets.soundcloud_oauth_relay.clone(),
            pending: false,
            authenticated: authed,
            account_name: secrets.soundcloud_account.clone(),
            auth_url: None,
            error: None,
        }
    }

    pub fn to_status(&self) -> SoundCloudOAuthStatus {
        SoundCloudOAuthStatus {
            session_id: self.session_id.clone(),
            pending: self.pending,
            authenticated: self.authenticated,
            account_name: self.account_name.clone(),
            auth_url: self.auth_url.clone(),
            error: self.error.clone(),
        }
    }
}

pub struct SecretStore {
    path: PathBuf,
    pub data: StoredSecrets,
}

impl SecretStore {
    pub fn load(app_data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&app_data_dir);
        let path = app_data_dir.join("aura_vault.json");

        // Миграция со старого identifier (com.aura.music -> com.flowly.music)
        if !path.exists() {
            if let Some(parent) = app_data_dir.parent() {
                let legacy = parent.join("com.aura.music").join("aura_vault.json");
                if legacy.exists() {
                    let _ = fs::copy(&legacy, &path);
                }
            }
        }

        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, data }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}
