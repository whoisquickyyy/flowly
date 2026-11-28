use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Yandex,
    SoundCloud,
    Local,
}

impl SourceKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            SourceKind::Yandex => "Яндекс Музыка",
            SourceKind::SoundCloud => "SoundCloud",
            SourceKind::Local => "Локальные файлы",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    Direct,
    Socks5,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub mode: ProxyMode,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    pub fn direct() -> Self {
        Self {
            mode: ProxyMode::Direct,
            host: String::new(),
            port: 1080,
            username: None,
            password: None,
        }
    }

    pub fn local_socks5(port: u16) -> Self {
        Self {
            mode: ProxyMode::Socks5,
            host: "127.0.0.1".to_string(),
            port,
            username: None,
            password: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub can_search: bool,
    pub can_stream_in_app: bool,
    pub can_remote_control: bool,
    pub can_seek: bool,
    pub can_control_volume: bool,
    pub can_read_library: bool,
    pub can_read_playlists: bool,
    pub auth_method: String,
    pub playback_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub source: SourceKind,
    pub connected: bool,
    pub account_name: Option<String>,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceStatus {
    pub source: SourceKind,
    pub name: String,
    pub proxy: ProxyConfig,
    pub is_reachable: Option<bool>,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub auth: AuthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork: String,
    pub duration: u32,
    pub source: SourceKind,
    pub genre: Option<String>,
    pub release_year: Option<u16>,
    pub playback_url: Option<String>,
    pub external_url: String,
    pub audio_quality: Option<String>,
    pub rating: Option<f32>,
    pub accent_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub source: Option<SourceKind>,
    pub is_local: bool,
    pub artwork: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySection {
    AllTracks,
    Albums,
    Artists,
    Playlists,
    LikedSongs,
    RecentlyPlayed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupedSearchResult {
    pub source: SourceKind,
    pub source_name: String,
    pub tracks: Vec<Track>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEffectsConfig {
    pub eq_enabled: bool,
    pub preset: String,
    pub bands: [f32; 10],
    #[serde(default)]
    pub preamp_db: f32,
    pub crossfade_sec: u8,
    pub normalize_volume: bool,
    pub bass_boost: u8,
    pub spatial_audio: bool,
}

impl Default for AudioEffectsConfig {
    fn default() -> Self {
        Self {
            eq_enabled: true,
            preset: "Flat".to_string(),
            bands: [0.0; 10],
            preamp_db: 0.0,
            crossfade_sec: 4,
            normalize_volume: true,
            bass_boost: 0,
            spatial_audio: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub current_track: Option<Track>,
    pub is_playing: bool,
    pub position_sec: u32,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub queue: Vec<Track>,
    #[serde(default)]
    pub shuffle_order: Vec<String>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            current_track: None,
            is_playing: false,
            position_sec: 0,
            volume: 80,
            shuffle: false,
            repeat: RepeatMode::All,
            queue: Vec::new(),
            shuffle_order: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRelease {
    pub id: String,
    pub title: String,
    pub artwork: String,
    pub year: Option<u16>,
    pub release_type: Option<String>,
    pub release_date: Option<String>,
    pub track_count: Option<u32>,
    pub external_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarArtist {
    pub id: String,
    pub name: String,
    pub artwork: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistYandexInfo {
    pub id: String,
    pub name: String,
    pub artwork: Option<String>,
    pub monthly_listeners: Option<u64>,
    pub latest_release: Option<ArtistRelease>,
    pub external_url: String,
    #[serde(default)]
    pub similar_artists: Vec<SimilarArtist>,
    #[serde(default)]
    pub albums: Vec<ArtistRelease>,
    #[serde(default)]
    pub singles: Vec<ArtistRelease>,
    #[serde(default)]
    pub popular_tracks: Vec<Track>,
    #[serde(default)]
    pub genres: Vec<String>,
}
