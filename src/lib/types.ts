export type SourceKind = 'yandex' | 'soundcloud' | 'local';
export type ProxyMode = 'direct' | 'socks5' | 'http';
export type RepeatMode = 'off' | 'all' | 'one';
export type BackgroundMode =
  | 'ambient'
  | 'blur-cover'
  | 'aurora'
  | 'spotlight'
  | 'graphite'
  | 'rain-glass'
  | 'starfield'
  | 'fog-grain'
  | 'dynamic-lyrics';
export type LayoutStyle = 'wave-dock' | 'modern-analog' | 'tv-mode';
export type MixMode = 'auto' | 'manual';
export type MixPreset = 'fade' | 'rise' | 'bass-swap' | 'echo-out' | 'dj-vocal';
export type WaveMood = 'mix' | 'energy' | 'chill' | 'discovery' | 'favorites';

export interface WaveConfig {
  sourceFilter: SourceKind | null;
  mood: WaveMood;
}

export type ManualTransitionStyle =
  | 'filter-cut'
  | 'echo-tail'
  | 'clean-whip'
  | 'vinyl-brake'
  | 'tape-whoosh'
  | 'off';

export interface MixConfig {
  enabled: boolean;
  mode: MixMode;
  preset: MixPreset;
  durationSec: number;
  eqSweep: boolean;
  manualTransition?: ManualTransitionStyle;
}
export type LibrarySection =
  | 'allTracks'
  | 'albums'
  | 'artists'
  | 'playlists'
  | 'likedSongs'
  | 'recentlyPlayed'
  | 'localFiles';

export interface ProxyConfig {
  mode: ProxyMode;
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
}

export interface ProviderCapabilities {
  can_search: boolean;
  can_stream_in_app: boolean;
  can_remote_control: boolean;
  can_seek: boolean;
  can_control_volume: boolean;
  can_read_library: boolean;
  can_read_playlists: boolean;
  auth_method: string;
  playback_note: string;
}

export interface AuthStatus {
  source: SourceKind;
  connected: boolean;
  account_name: string | null;
  capabilities: ProviderCapabilities;
}

export interface SourceStatus {
  source: SourceKind;
  name: string;
  proxy: ProxyConfig;
  is_reachable: boolean | null;
  latency_ms: number | null;
  last_error: string | null;
  auth: AuthStatus;
}

export interface Track {
  id: string;
  title: string;
  artist: string;
  album: string;
  artwork: string;
  duration: number;
  source: SourceKind;
  genre: string | null;
  releaseYear: number | null;
  playbackUrl: string | null;
  externalUrl: string;
  audioQuality: string | null;
  rating: number | null;
  accentColor: string;
}

export interface Playlist {
  id: string;
  title: string;
  description: string | null;
  source: SourceKind | null;
  isLocal: boolean;
  artwork: string | null;
  tracks: Track[];
}

export interface GroupedSearchResult {
  source: SourceKind;
  sourceName: string;
  tracks: Track[];
  error: string | null;
}

export interface AudioEffectsConfig {
  eq_enabled: boolean;
  preset: string;
  bands: number[];
  preamp_db?: number;
  crossfade_sec: number;
  normalize_volume: boolean;
  bass_boost: number;
  spatial_audio: boolean;
}

export interface LyricsLine {
  timeMs: number | null;
  text: string;
}

export interface LyricsResponse {
  synced: boolean;
  source: string;
  lines: LyricsLine[];
}

export interface NetworkLyricsSettings {
  doh_enabled: boolean;
  doh_provider: string;
  spicy_lyrics_key: string;
  discord_rpc_enabled: boolean;
  discord_client_id: string;
  close_to_tray: boolean;
  dynamic_theme_enabled: boolean;
  weather_city: string;
}

export interface WeatherAtmosphere {
  city: string;
  tempC: number;
  weatherKind: 'clear' | 'cloudy' | 'rain' | 'snow' | 'storm' | 'fog';
  summary: string;
}

export type ActivityCategory = 'flowly' | 'coding' | 'gaming' | 'video' | 'other';

export interface ActivityInfo {
  category: ActivityCategory;
  appName: string;
  title: string;
}

export interface ActivityConfig {
  enabled: boolean;
  codingPlaylistId: string;
  codingEqPreset: string;
  codingEco: boolean;
  gamingEqPreset: string;
  gamingEco: boolean;
  videoAutoPause: boolean;
  videoAutoResume: boolean;
  customGames: string;
}

export interface ArtistRelease {
  id: string;
  title: string;
  artwork: string;
  year: number | null;
  releaseType: string | null;
  releaseDate: string | null;
  trackCount: number | null;
  externalUrl: string;
}

export interface SimilarArtist {
  id: string;
  name: string;
  artwork: string | null;
}

export interface ArtistYandexInfo {
  id: string;
  name: string;
  artwork: string | null;
  monthlyListeners: number | null;
  latestRelease: ArtistRelease | null;
  externalUrl: string;
  similarArtists?: SimilarArtist[];
}

export type RecapPeriod = 'day' | 'week' | 'month' | 'all';

export interface RecapArtistStat {
  name: string;
  plays: number;
  totalSec: number;
  artwork: string;
}

export interface RecapTrackStat {
  track: Track;
  plays: number;
  totalSec: number;
}

export interface RecapGenreStat {
  genre: string;
  plays: number;
  percent: number;
}

export interface RecapSummary {
  badge: string;
  headline: string;
  description: string;
  accentColor: string;
}

export interface RecapDiscovery {
  artist: string;
  track: Track;
}

export interface ListeningRecap {
  period: RecapPeriod;
  totalPlays: number;
  uniqueTracks: number;
  uniqueArtists: number;
  totalSec: number;
  topGenre: string | null;
  genres: RecapGenreStat[];
  topArtists: RecapArtistStat[];
  topTracks: RecapTrackStat[];
  summary: RecapSummary;
  hourlyPlays?: number[];
  peakHour?: number | null;
  streakDays?: number;
  discovery?: RecapDiscovery | null;
}

export interface TrackSample {
  title: string;
  artist: string;
  relation: string;
  mbid: string;
}

export interface TrackSamplesResponse {
  samples: TrackSample[];
  whosampledUrl: string;
}

export type RadioEngine = 'edge' | 'piper' | 'system';

export interface RadioHostConfig {
  enabled: boolean;
  engine: RadioEngine;
  voice: 'male' | 'female';
  frequency: 2 | 3 | 5;
  city: string;
}

export interface RadioHostResult {
  text: string;
  audioDataUrl: string | null;
  usedEngine: string;
  error: string | null;
}

export type ShuffleMode = 'classic' | 'bpm-flow';

export interface PlayerState {
  current_track: Track | null;
  is_playing: boolean;
  position_sec: number;
  volume: number;
  shuffle: boolean;
  shuffle_mode?: ShuffleMode;
  repeat: RepeatMode;
  queue: Track[];
  shuffle_order?: string[];
}
