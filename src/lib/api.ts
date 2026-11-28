import { invoke } from '@tauri-apps/api/core';
import type {
  ActivityInfo,
  AudioEffectsConfig,
  ArtistYandexInfo,
  GroupedSearchResult,
  ListeningRecap,
  RadioHostResult,
  RecapPeriod,
  TrackSamplesResponse,
  WeatherAtmosphere,
  LibrarySection,
  LyricsResponse,
  NetworkLyricsSettings,
  PlayerState,
  Playlist,
  ProxyConfig,
  SourceKind,
  SourceStatus,
  Track,
} from './types';

export const isTauri = () =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function getLibrarySection(
  section: LibrarySection,
  sourceFilter: SourceKind | null
): Promise<Track[]> {
  return invoke<Track[]>('get_library_section', { section, sourceFilter });
}

export async function searchGrouped(
  query: string,
  sourceFilter: SourceKind | null
): Promise<GroupedSearchResult[]> {
  return invoke<GroupedSearchResult[]>('search_grouped', { query, sourceFilter });
}

export async function getPlayerState(): Promise<PlayerState> {
  return invoke<PlayerState>('get_player_state');
}

export async function playTrack(track: Track, queue?: Track[]): Promise<PlayerState> {
  return invoke<PlayerState>('play_track', { track, queue: queue ?? null });
}

export async function startMyWave(sourceFilter: SourceKind | null = null): Promise<PlayerState> {
  return invoke<PlayerState>('start_my_wave', { sourceFilter });
}

export async function getLikedIds(): Promise<string[]> {
  return invoke<string[]>('get_liked_ids');
}

export async function toggleLikeTrack(track: Track, liked: boolean): Promise<string[]> {
  return invoke<string[]>('toggle_like_track', { track, liked });
}

export async function togglePlayback(): Promise<PlayerState> {
  return invoke<PlayerState>('toggle_playback');
}

export async function nextTrack(manual = false): Promise<PlayerState> {
  return invoke<PlayerState>('next_track', { manual });
}

export async function revalidatePlaybackUrl(track: Track): Promise<string | null> {
  return invoke<string | null>('revalidate_playback_url', { track });
}

export async function warmUpNextTracks(count = 3): Promise<void> {
  return invoke<void>('warm_up_next_tracks', { count }).catch(() => {});
}

export async function topUpWave(): Promise<PlayerState> {
  return invoke<PlayerState>('top_up_wave');
}

export async function relayServerStart(port: number): Promise<{ address: string; port: number }> {
  return invoke('relay_server_start', { port });
}

export async function relayServerStop(): Promise<void> {
  return invoke<void>('relay_server_stop').catch(() => {});
}

export async function relayServerRunning(): Promise<boolean> {
  return invoke<boolean>('relay_server_running').catch(() => false);
}

export async function relayInviteNew(): Promise<string> {
  return invoke<string>('together_relay_invite_new');
}

export async function relayServerLocalIp(): Promise<string | null> {
  return invoke<string | null>('relay_server_local_ip');
}

export type StationKind = 'deep_cuts' | 'fresh_drops' | 'same_artist';

export async function startStation(kind: StationKind): Promise<PlayerState> {
  return invoke<PlayerState>('start_station', { kind });
}

export async function rotorTrackStarted(track: Track): Promise<void> {
  return invoke<void>('rotor_track_started', { track }).catch(() => {});
}

export async function prevTrack(): Promise<PlayerState> {
  return invoke<PlayerState>('prev_track');
}

export async function seekTo(positionSec: number): Promise<PlayerState> {
  return invoke<PlayerState>('seek_to', { positionSec });
}

export async function setVolume(volume: number): Promise<PlayerState> {
  return invoke<PlayerState>('set_volume', { volume });
}

export async function toggleShuffle(): Promise<PlayerState> {
  return invoke<PlayerState>('toggle_shuffle');
}

export async function cycleRepeat(): Promise<PlayerState> {
  return invoke<PlayerState>('cycle_repeat');
}

export async function getSourcesStatus(): Promise<SourceStatus[]> {
  return invoke<SourceStatus[]>('get_sources_status');
}

export async function updateSourceProxy(
  source: SourceKind,
  config: ProxyConfig
): Promise<SourceStatus[]> {
  return invoke<SourceStatus[]>('update_source_proxy', { source, config });
}

export async function testSourceConnection(source: SourceKind): Promise<SourceStatus> {
  return invoke<SourceStatus>('test_source_connection', { source });
}

export async function connectYandex(token: string): Promise<SourceStatus[]> {
  return invoke<SourceStatus[]>('connect_yandex', { token });
}

export interface SoundCloudOAuthStatus {
  sessionId: string | null;
  pending: boolean;
  authenticated: boolean;
  accountName: string | null;
  authUrl: string | null;
  error: string | null;
}

export interface SoundCloudPollResult {
  status: SoundCloudOAuthStatus;
  sources: SourceStatus[] | null;
}

export async function connectSoundCloud(
  oauthToken: string | null,
  clientId: string | null
): Promise<SourceStatus[]> {
  return invoke<SourceStatus[]>('connect_soundcloud', { oauthToken, clientId });
}

export async function startSoundCloudOAuth(
  relayUrl?: string | null
): Promise<SoundCloudOAuthStatus> {
  return invoke<SoundCloudOAuthStatus>('start_soundcloud_oauth', {
    relayUrl: relayUrl ?? null,
  });
}

export async function pollSoundCloudOAuth(): Promise<SoundCloudPollResult> {
  return invoke<SoundCloudPollResult>('poll_soundcloud_oauth');
}

export async function cancelSoundCloudOAuth(): Promise<SoundCloudOAuthStatus> {
  return invoke<SoundCloudOAuthStatus>('cancel_soundcloud_oauth');
}


export async function disconnectSource(source: SourceKind): Promise<SourceStatus[]> {
  return invoke<SourceStatus[]>('disconnect_source', { source });
}

export async function getAllPlaylists(): Promise<Playlist[]> {
  return invoke<Playlist[]>('get_all_playlists');
}

export async function createLocalPlaylist(
  title: string,
  description: string | null
): Promise<Playlist[]> {
  return invoke<Playlist[]>('create_local_playlist', { title, description });
}

export async function addTrackToPlaylist(
  playlistId: string,
  track: Track
): Promise<Playlist[]> {
  return invoke<Playlist[]>('add_track_to_playlist', { playlistId, track });
}

export async function removeTrackFromPlaylist(
  playlistId: string,
  trackId: string
): Promise<Playlist[]> {
  return invoke<Playlist[]>('remove_track_from_playlist', { playlistId, trackId });
}

export async function getAudioEffects(): Promise<AudioEffectsConfig> {
  return invoke<AudioEffectsConfig>('get_audio_effects');
}

export async function updateAudioEffects(
  config: AudioEffectsConfig
): Promise<AudioEffectsConfig> {
  return invoke<AudioEffectsConfig>('update_audio_effects', { config });
}

export async function getNetworkLyricsSettings(): Promise<NetworkLyricsSettings> {
  return invoke<NetworkLyricsSettings>('get_network_lyrics_settings');
}

export async function updateNetworkLyricsSettings(
  settings: NetworkLyricsSettings
): Promise<NetworkLyricsSettings> {
  return invoke<NetworkLyricsSettings>('update_network_lyrics_settings', { settings });
}

export async function testDohServer(provider: string): Promise<number> {
  return invoke<number>('test_doh_server', { provider });
}

export async function getTrackLyrics(track: Track): Promise<LyricsResponse> {
  return invoke<LyricsResponse>('get_track_lyrics', { track });
}

export async function getAudioProxyPort(): Promise<number> {
  if (!isTauri()) return 0;
  return invoke<number>('get_audio_proxy_port');
}

export async function openExternalUrl(url: string): Promise<void> {
  return invoke<void>('open_external_url', { url });
}

export async function resetWaveToLibrary(sourceFilter: SourceKind | null): Promise<PlayerState> {
  return invoke<PlayerState>('reset_wave_to_library', { sourceFilter });
}

export async function getArtistYandexInfo(artistName: string): Promise<ArtistYandexInfo | null> {
  if (!isTauri()) return null;
  return invoke<ArtistYandexInfo | null>('get_artist_yandex_info', { artistName });
}

export async function getArtistPreviewTrack(artistId: string, artistName: string): Promise<Track | null> {
  if (!isTauri()) return null;
  return invoke<Track | null>('get_artist_preview_track', { artistId, artistName });
}

export async function getYandexAlbumTracks(albumId: string): Promise<Track[]> {
  if (!isTauri()) return [];
  return invoke<Track[]>('get_yandex_album_tracks', { albumId });
}

export async function recordListen(track: Track, listenedSec: number): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>('record_listen', { track, listenedSec });
}

export async function getListeningRecap(period: RecapPeriod): Promise<ListeningRecap> {
  const tzOffsetMin = new Date().getTimezoneOffset();
  return invoke<ListeningRecap>('get_listening_recap', { period, tzOffsetMin });
}

export async function getTrackSamples(artist: string, title: string): Promise<TrackSamplesResponse> {
  if (!isTauri()) return { samples: [], whosampledUrl: '' };
  return invoke<TrackSamplesResponse>('get_track_samples', { artist, title });
}

export async function preloadNextTrack(): Promise<Track | null> {
  if (!isTauri()) return null;
  return invoke<Track | null>('preload_next_track');
}

export async function generateRadioHostLine(
  track: Track,
  engine: string,
  voice: string,
  city: string,
  isWave: boolean
): Promise<RadioHostResult> {
  const localHour = new Date().getHours();
  return invoke<RadioHostResult>('generate_radio_host_line', {
    track,
    engine,
    voice,
    city,
    localHour,
    isWave,
  });
}

export async function installPiperTts(): Promise<string> {
  return invoke<string>('install_piper_tts');
}

export async function getDynamicAtmosphere(city: string): Promise<WeatherAtmosphere | null> {
  if (!isTauri() || !city.trim()) return null;
  return invoke<WeatherAtmosphere | null>('get_dynamic_atmosphere', { city });
}

export async function getActiveWindowInfo(customGames: string[]): Promise<ActivityInfo> {
  if (!isTauri()) return { category: 'other', appName: '', title: '' };
  return invoke<ActivityInfo>('get_active_window_info', { customGames });
}

export async function dislikeTrack(track: Track): Promise<PlayerState> {
  return invoke<PlayerState>('dislike_track', { track });
}

export async function getLocalFolders(): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>('get_local_folders');
}

export async function pickAndAddLocalFolder(): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>('pick_and_add_local_folder');
}

export async function removeLocalFolder(path: string): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>('remove_local_folder', { path });
}

export async function scanLocalTracks(): Promise<Track[]> {
  if (!isTauri()) return [];
  return invoke<Track[]>('scan_local_tracks');
}

export async function updateSmtcState(params: {
  title: string;
  artist: string;
  album: string;
  coverUrl: string | null;
  durationSec: number;
  playing: boolean;
  positionSec: number;
}): Promise<void> {
  if (!isTauri()) return;
  await invoke('update_smtc_state', params).catch(() => {});
}

export async function setTrackCacheEnabled(enabled: boolean): Promise<void> {
  if (!isTauri()) return;
  await invoke('set_track_cache_enabled', { enabled }).catch(() => {});
}

export async function reportWaveFeedback(
  track: Track,
  positive: boolean,
  playedSec?: number,
): Promise<void> {
  if (!isTauri()) return;
  await invoke('report_wave_feedback', { track, positive, playedSec: playedSec ?? null }).catch(() => {});
}

