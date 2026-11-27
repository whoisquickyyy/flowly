use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::auth::{SecretStore, SoundCloudOAuthState};
use crate::discord_rpc::DiscordRpcHandle;
use crate::models::{AudioEffectsConfig, PlayerState, SourceKind, Track};
use crate::providers::ProviderRegistry;
use crate::radio_host::CachedWeather;
use crate::recap::RecapStore;

pub struct AppState {
    pub player: RwLock<PlayerState>,
    pub registry: Arc<RwLock<ProviderRegistry>>,
    pub effects: RwLock<AudioEffectsConfig>,
    pub vault: RwLock<SecretStore>,
    pub recap: RwLock<RecapStore>,
    pub app_data_dir: PathBuf,
    pub weather_cache: RwLock<Option<CachedWeather>>,
    pub audio_proxy_port: u16,
    pub wave_active: RwLock<bool>,
    pub wave_source: RwLock<Option<SourceKind>>,
    pub play_history: RwLock<Vec<Track>>,
    pub forward_history: RwLock<Vec<Track>>,
    pub shuffle_bag: RwLock<Vec<String>>,
    pub wave_weights: RwLock<std::collections::HashMap<String, f32>>,
    pub discord_rpc: DiscordRpcHandle,
    pub track_cache: crate::track_cache::TrackCache,
    pub audio_engine: Arc<std::sync::Mutex<Option<crate::audio::KitharaEngine>>>,
    pub sc_oauth: Arc<RwLock<SoundCloudOAuthState>>,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf, audio_proxy_port: u16) -> Self {
        let track_cache = crate::track_cache::TrackCache::new(&app_data_dir);
        let store = SecretStore::load(app_data_dir.clone());
        let recap_store = RecapStore::load(app_data_dir.clone());
        crate::network::doh::set_doh_config(
            store.data.doh_enabled.unwrap_or(true),
            store.data.doh_provider.as_deref().unwrap_or("cloudflare"),
        );
        let registry = ProviderRegistry::from_secrets(&store.data);

        let mut player = PlayerState::default();
        if let Some(v) = store.data.volume {
            player.volume = v.min(100);
        }
        if let Some(sh) = store.data.shuffle {
            player.shuffle = sh;
        }
        if let Some(sm) = store.data.shuffle_mode {
            player.shuffle_mode = sm;
        }
        if let Some(rp) = store.data.repeat {
            player.repeat = rp;
        }
        if let Some(mut last) = store.data.last_track.clone() {
            if last.source != SourceKind::Local {
                last.playback_url = None;
            }
            player.current_track = Some(last.clone());
            player.queue = if store.data.last_queue.is_empty() {
                if store.data.liked_tracks.is_empty() {
                    vec![last]
                } else {
                    store.data.liked_tracks.clone()
                }
            } else if store.data.last_queue.len() <= 45
                && store.data.liked_tracks.len() > store.data.last_queue.len()
            {
                store.data.liked_tracks.clone()
            } else {
                store.data.last_queue.clone()
            };
        } else if !store.data.liked_tracks.is_empty() {
            player.current_track = Some(store.data.liked_tracks[0].clone());
            player.queue = store.data.liked_tracks.clone();
        }

        let mut init_bag = Vec::new();
        if player.shuffle && player.queue.len() > 1 {
            init_bag = crate::commands::build_shuffle_bag(
                &player.queue,
                player.current_track.as_ref(),
                &[],
                player.shuffle_mode,
            );
            player.shuffle_order = init_bag.iter().rev().cloned().collect();
        }

        let effects = store.data.effects.clone().unwrap_or_default();
        let discord_rpc = DiscordRpcHandle::spawn(
            store.data.discord_rpc_enabled.unwrap_or(true),
            store.data.discord_client_id.clone().unwrap_or_default(),
        );

        let sc_oauth = SoundCloudOAuthState::from_secrets(&store.data);

        Self {
            player: RwLock::new(player),
            registry: Arc::new(RwLock::new(registry)),
            effects: RwLock::new(effects),
            vault: RwLock::new(store),
            recap: RwLock::new(recap_store),
            app_data_dir,
            weather_cache: RwLock::new(None),
            audio_proxy_port,
            wave_active: RwLock::new(false),
            wave_source: RwLock::new(None),
            play_history: RwLock::new(Vec::new()),
            forward_history: RwLock::new(Vec::new()),
            shuffle_bag: RwLock::new(init_bag),
            wave_weights: RwLock::new(std::collections::HashMap::new()),
            discord_rpc,
            track_cache,
            audio_engine: Arc::new(std::sync::Mutex::new(
                crate::audio::KitharaEngine::new().ok(),
            )),
            sc_oauth: Arc::new(RwLock::new(sc_oauth)),
        }
    }
}
