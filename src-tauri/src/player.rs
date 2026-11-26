use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::auth::SecretStore;
use crate::discord_rpc::DiscordRpcHandle;
use crate::models::{AudioEffectsConfig, PlayerState, SourceKind, Track};
use crate::providers::{demo_spotify_tracks, ProviderRegistry};
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
    pub discord_rpc: DiscordRpcHandle,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf, audio_proxy_port: u16) -> Self {
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
        if let Some(last) = store.data.last_track.clone() {
            player.current_track = Some(last);
            player.queue = if store.data.last_queue.is_empty() {
                if store.data.liked_tracks.is_empty() {
                    demo_spotify_tracks()
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
        } else {
            let initial_tracks = demo_spotify_tracks();
            if let Some(first) = initial_tracks.first().cloned() {
                player.current_track = Some(first);
                player.queue = initial_tracks;
            }
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
            discord_rpc,
        }
    }
}
