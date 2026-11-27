pub mod soundcloud;
pub mod yandex;

use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use rand::seq::SliceRandom;
use reqwest::Client;
use thiserror::Error;

use crate::auth::StoredSecrets;
use crate::models::{
    AuthStatus, GroupedSearchResult, LibrarySection, Playlist, ProviderCapabilities, ProxyConfig,
    SourceKind, SourceStatus, Track,
};
use crate::network::proxy::NetworkError;

use soundcloud::SoundCloudProvider;
use yandex::YandexMusicProvider;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Ошибка сети: {0}")]
    Network(#[from] NetworkError),
    #[error("Ошибка HTTP-запроса: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Требуется авторизация в сервисе")]
    Unauthorized,
    #[error("Ошибка API: {0}")]
    Api(String),
}

#[async_trait]
pub trait MusicProvider: Send + Sync {
    #[allow(dead_code)]
    fn kind(&self) -> SourceKind;
    fn health_check_url(&self) -> &'static str;
    fn proxy_config(&self) -> &ProxyConfig;
    fn set_proxy_config(&mut self, config: ProxyConfig) -> Result<(), ProviderError>;
    fn client(&self) -> &Client;
    fn capabilities(&self) -> ProviderCapabilities;
    fn auth_status(&self) -> AuthStatus;

    async fn search(&self, query: &str) -> Result<Vec<Track>, ProviderError>;
    async fn resolve_playback_url(&self, track_id: &str) -> Result<Option<String>, ProviderError>;

    async fn check_connection(&self) -> Result<u64, ProviderError> {
        let start = Instant::now();
        let resp = self.client().head(self.health_check_url()).send().await?;
        let _ = resp.status();
        Ok(start.elapsed().as_millis() as u64)
    }
}

pub struct ProviderRegistry {
    pub yandex: YandexMusicProvider,
    pub soundcloud: SoundCloudProvider,
    statuses: HashMap<SourceKind, (Option<bool>, Option<u64>, Option<String>)>,
}

impl ProviderRegistry {
    pub fn from_secrets(secrets: &StoredSecrets) -> Self {
        let yandex_proxy = secrets
            .proxies
            .get(&SourceKind::Yandex)
            .cloned()
            .unwrap_or_else(ProxyConfig::direct);
        let sc_proxy = secrets
            .proxies
            .get(&SourceKind::SoundCloud)
            .cloned()
            .unwrap_or_else(|| ProxyConfig::local_socks5(1080));

        Self {
            yandex: YandexMusicProvider::new(
                yandex_proxy,
                secrets.yandex_token.clone(),
                secrets.yandex_account.clone(),
            ),
            soundcloud: SoundCloudProvider::new(
                sc_proxy,
                secrets.soundcloud_token.clone(),
                secrets.soundcloud_client_id.clone(),
                secrets.soundcloud_account.clone(),
            ),
            statuses: HashMap::new(),
        }
    }

    pub fn update_proxy(
        &mut self,
        source: SourceKind,
        config: ProxyConfig,
    ) -> Result<(), ProviderError> {
        match source {
            SourceKind::Yandex => self.yandex.set_proxy_config(config)?,
            SourceKind::SoundCloud => self.soundcloud.set_proxy_config(config)?,
            SourceKind::Local => {}
        }
        self.statuses.remove(&source);
        Ok(())
    }

    pub async fn test_connection(&mut self, source: SourceKind) -> SourceStatus {
        let res = match source {
            SourceKind::Yandex => self.yandex.check_connection().await,
            SourceKind::SoundCloud => self.soundcloud.check_connection().await,
            SourceKind::Local => Ok(1),
        };
        match res {
            Ok(ms) => {
                self.statuses.insert(source, (Some(true), Some(ms), None));
            }
            Err(err) => {
                self.statuses
                    .insert(source, (Some(false), None, Some(err.to_string())));
            }
        }
        self.status_for(source)
    }

    pub fn all_statuses(&self) -> Vec<SourceStatus> {
        [SourceKind::Yandex, SourceKind::SoundCloud]
            .into_iter()
            .map(|k| self.status_for(k))
            .collect()
    }

    pub fn status_for(&self, source: SourceKind) -> SourceStatus {
        let (proxy, auth) = match source {
            SourceKind::Yandex => (self.yandex.proxy_config().clone(), self.yandex.auth_status()),
            SourceKind::SoundCloud => (
                self.soundcloud.proxy_config().clone(),
                self.soundcloud.auth_status(),
            ),
            SourceKind::Local => (
                ProxyConfig::direct(),
                crate::models::AuthStatus {
                    source: SourceKind::Local,
                    connected: true,
                    account_name: Some("Локальная папка".into()),
                    capabilities: crate::models::ProviderCapabilities {
                        can_search: true,
                        can_stream_in_app: true,
                        can_remote_control: false,
                        can_seek: true,
                        can_control_volume: true,
                        can_read_library: true,
                        can_read_playlists: false,
                        auth_method: "local".into(),
                        playback_note: "Локальные аудиофайлы".into(),
                    },
                },
            ),
        };
        let (is_reachable, latency_ms, last_error) = self
            .statuses
            .get(&source)
            .cloned()
            .unwrap_or((None, None, None));

        SourceStatus {
            source,
            name: source.display_name().to_string(),
            proxy,
            is_reachable,
            latency_ms,
            last_error,
            auth,
        }
    }

    pub async fn search_grouped(
        &self,
        query: &str,
        source_filter: Option<SourceKind>,
    ) -> Vec<GroupedSearchResult> {
        let q = query.trim();
        let mut out = Vec::new();

        let targets = match source_filter {
            Some(s) => vec![s],
            None => vec![SourceKind::Yandex, SourceKind::SoundCloud],
        };

        for src in targets {
            let res = match src {
                SourceKind::Yandex => self.yandex.search(q).await,
                SourceKind::SoundCloud => self.soundcloud.search(q).await,
                SourceKind::Local => Ok(Vec::new()),
            };

            match res {
                Ok(tracks) => out.push(GroupedSearchResult {
                    source: src,
                    source_name: src.display_name().to_string(),
                    tracks,
                    error: None,
                }),
                Err(ProviderError::Unauthorized) => {
                    out.push(GroupedSearchResult {
                        source: src,
                        source_name: src.display_name().to_string(),
                        tracks: Vec::new(),
                        error: Some("Вы не авторизованы".into()),
                    });
                }
                Err(e) => out.push(GroupedSearchResult {
                    source: src,
                    source_name: src.display_name().to_string(),
                    tracks: Vec::new(),
                    error: Some(e.to_string()),
                }),
            }
        }
        out
    }

    pub async fn fetch_library(
        &mut self,
        section: LibrarySection,
        source_filter: Option<SourceKind>,
    ) -> Vec<Track> {
        let include_ym = source_filter.is_none() || source_filter == Some(SourceKind::Yandex);
        let include_sc = source_filter.is_none() || source_filter == Some(SourceKind::SoundCloud);

        let ym_fut = async {
            if !include_ym || !self.yandex.auth_status().connected {
                return Vec::new();
            }
            let res = match section {
                LibrarySection::Albums => self.yandex.fetch_liked_albums().await,
                LibrarySection::RecentlyPlayed => Ok(Vec::new()),
                _ => self.yandex.fetch_liked_tracks().await,
            };
            res.unwrap_or_default()
        };

        let sc_fut = async {
            if !include_sc || !self.soundcloud.auth_status().connected {
                return Vec::new();
            }
            let res = match section {
                LibrarySection::RecentlyPlayed => self.soundcloud.fetch_recently_played().await,
                _ => self.soundcloud.fetch_liked_tracks().await,
            };
            res.unwrap_or_default()
        };

        let (mut ym, mut sc) = tokio::join!(ym_fut, sc_fut);
        let mut tracks = Vec::with_capacity(ym.len() + sc.len());
        tracks.append(&mut ym);
        tracks.append(&mut sc);
        tracks
    }

    pub async fn fetch_remote_playlists(&mut self) -> Vec<Playlist> {
        let mut all = Vec::new();
        if self.yandex.auth_status().connected {
            if let Ok(mut ym) = self.yandex.fetch_playlists().await {
                all.append(&mut ym);
            }
        }
        if self.soundcloud.auth_status().connected {
            if let Ok(mut sc) = self.soundcloud.fetch_playlists().await {
                all.append(&mut sc);
            }
        }
        all
    }

    pub async fn resolve_playback_url(
        &self,
        source: SourceKind,
        track_id: &str,
    ) -> Result<Option<String>, ProviderError> {
        match source {
            SourceKind::Yandex => self.yandex.resolve_playback_url(track_id).await,
            SourceKind::SoundCloud => self.soundcloud.resolve_playback_url(track_id).await,
            SourceKind::Local => Ok(None),
        }
    }

    pub async fn generate_wave_queue_excluding(
        &mut self,
        source_filter: Option<SourceKind>,
        exclude_ids: &std::collections::HashSet<String>,
        ym_seed: Option<&str>,
    ) -> Vec<Track> {
        let mut ym = Vec::new();
        let mut sc = Vec::new();

        let want_ym = source_filter.is_none() || source_filter == Some(SourceKind::Yandex);
        let want_sc = source_filter.is_none() || source_filter == Some(SourceKind::SoundCloud);

        if want_ym && self.yandex.auth_status().connected {
            if let Ok(list) = self
                .yandex
                .fetch_wave_tracks_excluding(exclude_ids, ym_seed)
                .await
            {
                ym = list;
            }
        }
        if want_sc && self.soundcloud.auth_status().connected {
            if let Ok(mut list) = self.soundcloud.fetch_wave_tracks().await {
                list.retain(|t| !exclude_ids.contains(&t.id));
                sc = list;
            }
        }

        let mut rng = rand::thread_rng();
        // Яндекс-ротор отдаёт уже отсортированную последовательность — не перемешиваем её
        sc.shuffle(&mut rng);

        // Перемешиваем треки со всех подключённых площадок (round-robin) с дедупликацией
        let mut mixed = Vec::new();
        let mut seen = exclude_ids.clone();
        let max_len = ym.len().max(sc.len());
        for i in 0..max_len {
            if i < ym.len() && seen.insert(ym[i].id.clone()) {
                mixed.push(ym[i].clone());
            }
            if i < sc.len() && seen.insert(sc[i].id.clone()) {
                mixed.push(sc[i].clone());
            }
        }

        mixed.truncate(60);
        mixed
    }

    #[allow(dead_code)]
    pub async fn generate_wave_queue(&mut self, source_filter: Option<SourceKind>) -> Vec<Track> {
        let empty = std::collections::HashSet::new();
        self.generate_wave_queue_excluding(source_filter, &empty, None)
            .await
    }
}
