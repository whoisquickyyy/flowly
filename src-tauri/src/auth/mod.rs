use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::{AudioEffectsConfig, Playlist, ProxyConfig, RepeatMode, SourceKind, Track};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredSecrets {
    pub yandex_token: Option<String>,
    pub yandex_account: Option<String>,
    pub spotify_client_id: Option<String>,
    pub spotify_access_token: Option<String>,
    pub spotify_refresh_token: Option<String>,
    pub spotify_account: Option<String>,
    pub soundcloud_token: Option<String>,
    pub soundcloud_client_id: Option<String>,
    pub soundcloud_account: Option<String>,
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

pub fn generate_pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}
