use std::process::Command;

use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::auth::generate_pkce_pair;
use crate::lyrics::{fetch_lyrics, LyricsResponse};
use crate::models::{
    AudioEffectsConfig, GroupedSearchResult, LibrarySection, PlayerState, Playlist, ProxyConfig,
    RepeatMode, ShuffleMode, SourceKind, SourceStatus, Track,
};
use crate::player::AppState;
use crate::providers::MusicProvider;

#[tauri::command]
pub async fn get_library_section(
    state: State<'_, AppState>,
    section: LibrarySection,
    source_filter: Option<SourceKind>,
) -> Result<Vec<Track>, String> {
    let mut remote = {
        let mut registry = state.registry.write().await;
        registry.fetch_library(section, source_filter).await
    };

    if matches!(section, LibrarySection::LikedSongs | LibrarySection::AllTracks) {
        let vault = state.vault.read().await;
        let local_liked: Vec<Track> = vault
            .data
            .liked_tracks
            .iter()
            .filter(|t| source_filter.is_none() || source_filter == Some(t.source))
            .cloned()
            .collect();

        if !local_liked.is_empty() {
            let mut merged = Vec::with_capacity(local_liked.len() + remote.len());
            let mut seen = std::collections::HashSet::<String>::new();
            for t in local_liked.into_iter().chain(remote.into_iter()) {
                if seen.insert(t.id.clone()) {
                    merged.push(t);
                }
            }
            remote = merged;
        }
    }

    Ok(remote)
}

#[tauri::command]
pub async fn search_grouped(
    state: State<'_, AppState>,
    query: String,
    source_filter: Option<SourceKind>,
) -> Result<Vec<GroupedSearchResult>, String> {
    let registry = state.registry.read().await;
    Ok(registry.search_grouped(&query, source_filter).await)
}

#[tauri::command]
pub async fn get_player_state(state: State<'_, AppState>) -> Result<PlayerState, String> {
    let player = state.player.read().await;
    Ok(player.clone())
}

#[tauri::command]
pub async fn play_track(
    state: State<'_, AppState>,
    track: Track,
    queue: Option<Vec<Track>>,
) -> Result<PlayerState, String> {
    play_track_inner(&state, track, queue, true).await
}

fn estimate_track_bpm(t: &Track) -> f32 {
    let g = t.genre.as_deref().unwrap_or("").to_lowercase();
    let mut h: u32 = 2166136261;
    for b in t.id.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    if g.contains("dnb") || g.contains("drum") || g.contains("jungle") {
        168.0 + (h % 10) as f32
    } else if g.contains("phonk") || g.contains("techno") || g.contains("hard") || g.contains("metal") || g.contains("punk") {
        132.0 + (h % 18) as f32
    } else if g.contains("rap") || g.contains("hip") || g.contains("trap") || g.contains("drill") {
        122.0 + (h % 24) as f32
    } else if g.contains("house") || g.contains("dance") || g.contains("electronic") || g.contains("edm") {
        120.0 + (h % 12) as f32
    } else if g.contains("rock") || g.contains("indie") || g.contains("alt") {
        112.0 + (h % 22) as f32
    } else if g.contains("chill") || g.contains("lo-fi") || g.contains("ambient") || g.contains("jazz") {
        82.0 + (h % 18) as f32
    } else {
        102.0 + (h % 26) as f32
    }
}

fn bpm_compat_score(a: f32, b: f32) -> f32 {
    let ratios = [a / b, a / (b * 2.0), (a * 2.0) / b];
    let best_diff = ratios
        .iter()
        .map(|r| (1.0 - r).abs())
        .fold(f32::MAX, f32::min);
    if best_diff <= 0.08 {
        1.5
    } else if best_diff <= 0.15 {
        0.9
    } else {
        0.0
    }
}

/// Заранее просчитанный шафл по всей очереди:
/// - `Classic`: честный Fisher-Yates с anti-repeat и мягким разносом подряд идущих артистов
/// - `BpmFlow`: плавная волна по BPM (включая кратность 2x/0.5x) с разносом артистов и альбомов
/// Возвращает стек (последний элемент будет взят первым через .pop()).
pub(crate) fn build_shuffle_bag(
    queue: &[Track],
    curr: Option<&Track>,
    history: &[Track],
    mode: ShuffleMode,
) -> Vec<String> {
    use rand::seq::SliceRandom;
    use rand::Rng;
    let curr_id = curr.map(|c| c.id.as_str());
    let recent_ids: std::collections::HashSet<&str> = history
        .iter()
        .rev()
        .take(50)
        .map(|t| t.id.as_str())
        .collect();

    let mut fresh_pool: Vec<&Track> = Vec::new();
    let mut recent_pool: Vec<&Track> = Vec::new();
    for t in queue {
        if curr_id == Some(t.id.as_str()) {
            continue;
        }
        if recent_ids.contains(t.id.as_str()) {
            recent_pool.push(t);
        } else {
            fresh_pool.push(t);
        }
    }

    let mut rng = rand::thread_rng();
    let mut ordered: Vec<String> = Vec::with_capacity(fresh_pool.len() + recent_pool.len());

    if mode == ShuffleMode::Classic {
        fresh_pool.shuffle(&mut rng);
        recent_pool.shuffle(&mut rng);
        for mut pool in [fresh_pool, recent_pool] {
            // Лёгкий разнос: если соседние треки одного артиста, пробуем поменять со следующим
            for i in 1..pool.len() {
                if pool[i].artist.eq_ignore_ascii_case(&pool[i - 1].artist) {
                    if let Some(j) = ((i + 1)..pool.len().min(i + 8))
                        .find(|&k| !pool[k].artist.eq_ignore_ascii_case(&pool[i - 1].artist))
                    {
                        pool.swap(i, j);
                    }
                }
            }
            ordered.extend(pool.into_iter().map(|t| t.id.clone()));
        }
        ordered.reverse();
        return ordered;
    }

    let mut last_bpm = curr.map(estimate_track_bpm).unwrap_or(118.0);
    let mut recent_artists: std::collections::VecDeque<String> = std::collections::VecDeque::with_capacity(3);
    if let Some(c) = curr {
        recent_artists.push_back(c.artist.to_lowercase());
    }
    let mut last_album = curr.map(|c| c.album.to_lowercase()).unwrap_or_default();

    for mut pool in [fresh_pool, recent_pool] {
        while !pool.is_empty() {
            let sample_len = pool.len().min(32);
            let mut scored: Vec<(usize, f32)> = Vec::with_capacity(sample_len);
            for _ in 0..sample_len {
                let idx = rng.gen_range(0..pool.len());
                let cand = pool[idx];
                let artist_lc = cand.artist.to_lowercase();
                let album_lc = cand.album.to_lowercase();
                let cand_bpm = estimate_track_bpm(cand);

                let mut score = rng.gen_range(0.0_f32..1.6_f32);
                score += bpm_compat_score(last_bpm, cand_bpm);
                if recent_artists.iter().any(|a| a == &artist_lc) {
                    score -= 4.5;
                }
                if !album_lc.is_empty() && album_lc == last_album {
                    score -= 1.5;
                }
                scored.push((idx, score));
            }
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_k = scored.len().min(3);
            let chosen_idx = scored[rng.gen_range(0..top_k)].0;
            let chosen = pool.swap_remove(chosen_idx);

            ordered.push(chosen.id.clone());
            last_bpm = estimate_track_bpm(chosen);
            last_album = chosen.album.to_lowercase();
            if recent_artists.len() >= 2 {
                recent_artists.pop_front();
            }
            recent_artists.push_back(chosen.artist.to_lowercase());
        }
    }

    ordered.reverse();
    ordered
}

async fn play_track_inner(
    state: &AppState,
    mut track: Track,
    queue: Option<Vec<Track>>,
    clear_forward: bool,
) -> Result<PlayerState, String> {
    if clear_forward {
        state.forward_history.write().await.clear();
    }
    if track.source == SourceKind::Spotify {
        if track.playback_url.is_none() {
            let registry = state.registry.read().await;
            if registry.spotify.auth_status().connected {
                let _ = registry.spotify.connect_play(&track.id).await;
            }
        }
    } else if track.source != SourceKind::Local {
        let registry = state.registry.read().await;
        if let Ok(Some(url)) = registry.resolve_playback_url(track.source, &track.id).await {
            track.playback_url = Some(url);
        }
    }

    if let Some(q) = queue {
        *state.wave_active.write().await = false;
        let hist = state.play_history.read().await.clone();
        let mut player = state.player.write().await;
        player.queue = q;
        if player.shuffle && player.queue.len() > 1 {
            let fresh = build_shuffle_bag(&player.queue, Some(&track), &hist, player.shuffle_mode);
            player.shuffle_order = fresh.iter().rev().cloned().collect();
            *state.shuffle_bag.write().await = fresh;
        } else {
            player.shuffle_order.clear();
            state.shuffle_bag.write().await.clear();
        }
    } else {
        let mut player = state.player.write().await;
        if !player.queue.iter().any(|t| t.id == track.id) {
            player.queue.push(track.clone());
        }
    }
    let snapshot = {
        let mut player = state.player.write().await;
        player.current_track = Some(track.clone());
        player.is_playing = true;
        player.position_sec = 0;

        {
            let mut vault = state.vault.write().await;
            vault.data.last_track = Some(track);
            vault.data.last_queue = player.queue.iter().take(2500).cloned().collect();
            let _ = vault.save();
        }
        player.clone()
    };

    state.discord_rpc.update_from_player(&snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn toggle_playback(state: State<'_, AppState>) -> Result<PlayerState, String> {
    let mut player = state.player.write().await;
    if let Some(source) = player.current_track.as_ref().map(|t| t.source) {
        player.is_playing = !player.is_playing;
        if source == SourceKind::Spotify {
            let registry = state.registry.read().await;
            if registry.spotify.auth_status().connected {
                let endpoint = if player.is_playing { "play" } else { "pause" };
                let _ = registry.spotify.connect_command("PUT", endpoint).await;
            }
        }
    }
    let snapshot = player.clone();
    state.discord_rpc.update_from_player(&snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn next_track(state: State<'_, AppState>) -> Result<PlayerState, String> {
    // Если пользователь ранее нажал «Назад» — возвращаем его по forward_history
    let fwd_track = { state.forward_history.write().await.pop() };
    if let Some(fwd) = fwd_track {
        if let Some(curr) = state.player.read().await.current_track.clone() {
            let mut hist = state.play_history.write().await;
            if hist.last().map(|t| &t.id) != Some(&curr.id) {
                hist.push(curr);
                if hist.len() > 100 {
                    hist.remove(0);
                }
            }
        }
        return play_track_inner(&state, fwd, None, false).await;
    }

    let (should_refill_wave, ym_seed, mut exclude_ids) = {
        let wave_on = *state.wave_active.read().await;
        if wave_on {
            let player = state.player.read().await;
            let curr_idx = player
                .current_track
                .as_ref()
                .and_then(|curr| player.queue.iter().position(|t| t.id == curr.id))
                .unwrap_or(0);
            let need = curr_idx + 8 >= player.queue.len();
            let seed = player
                .queue
                .iter()
                .rev()
                .find(|t| t.source == SourceKind::Yandex)
                .map(|t| t.id.clone());
            let mut ex: std::collections::HashSet<String> =
                player.queue.iter().map(|t| t.id.clone()).collect();
            for h in state.play_history.read().await.iter() {
                ex.insert(h.id.clone());
            }
            (need, seed, ex)
        } else {
            (false, None, std::collections::HashSet::new())
        }
    };

    if should_refill_wave {
        {
            let vault = state.vault.read().await;
            exclude_ids.extend(vault.data.disliked_ids.iter().cloned());
        }
        let wave_src = *state.wave_source.read().await;
        let more = {
            let mut registry = state.registry.write().await;
            registry
                .generate_wave_queue_excluding(wave_src, &exclude_ids, ym_seed.as_deref())
                .await
        };
        let mut player = state.player.write().await;
        let existing: std::collections::HashSet<String> =
            player.queue.iter().map(|t| t.id.clone()).collect();
        let mut added_ids = Vec::new();
        for t in more {
            if !existing.contains(&t.id) && !exclude_ids.contains(&t.id) {
                added_ids.push(t.id.clone());
                player.queue.push(t);
            }
        }
        if player.shuffle && !added_ids.is_empty() {
            let mut bag = state.shuffle_bag.write().await;
            bag.splice(0..0, added_ids);
            player.shuffle_order = bag.iter().rev().cloned().collect();
        }
        // Ограничим размер очереди сверху, удаляя старые пройденные треки в начале
        if player.queue.len() > 120 {
            let drain_count = player.queue.len() - 90;
            player.queue.drain(0..drain_count);
        }
    }

    let next_item = {
        let mut player = state.player.write().await;
        if player.queue.is_empty() {
            return Ok(player.clone());
        }
        if let Some(curr) = player.current_track.clone() {
            let mut hist = state.play_history.write().await;
            if hist.last().map(|t| &t.id) != Some(&curr.id) {
                hist.push(curr);
                if hist.len() > 100 {
                    hist.remove(0);
                }
            }
        }
        let curr_id = player.current_track.as_ref().map(|c| c.id.clone());
        let curr_idx = curr_id
            .as_ref()
            .and_then(|id| player.queue.iter().position(|t| &t.id == id));

        let wave_on = *state.wave_active.read().await;
        if player.shuffle && !wave_on && player.queue.len() > 1 {
            let hist = state.play_history.read().await.clone();
            let mut bag = state.shuffle_bag.write().await;
            // Удаляем из мешка id, которых больше нет в очереди или которые равны текущему треку
            let queue_ids: std::collections::HashSet<&str> =
                player.queue.iter().map(|t| t.id.as_str()).collect();
            bag.retain(|id| queue_ids.contains(id.as_str()) && curr_id.as_deref() != Some(id.as_str()));

            if bag.is_empty() {
                *bag = build_shuffle_bag(
                    &player.queue,
                    player.current_track.as_ref(),
                    &hist,
                    player.shuffle_mode,
                );
            }

            let picked_id = bag.pop();
            player.shuffle_order = bag.iter().rev().cloned().collect();
            picked_id
                .and_then(|pid| player.queue.iter().find(|t| t.id == pid).cloned())
                .unwrap_or_else(|| player.queue[0].clone())
        } else {
            let next_idx = match curr_idx {
                Some(i) => {
                    if player.repeat == RepeatMode::Off && i + 1 >= player.queue.len() {
                        player.is_playing = false;
                        return Ok(player.clone());
                    }
                    (i + 1) % player.queue.len()
                }
                None => 0,
            };
            player.queue[next_idx].clone()
        }
    };
    play_track_inner(&state, next_item, None, false).await
}

#[tauri::command]
pub async fn prev_track(state: State<'_, AppState>) -> Result<PlayerState, String> {
    let prev_from_hist = { state.play_history.write().await.pop() };
    if let Some(prev) = prev_from_hist {
        if let Some(curr) = state.player.read().await.current_track.clone() {
            let mut fwd = state.forward_history.write().await;
            if fwd.last().map(|t| &t.id) != Some(&curr.id) {
                fwd.push(curr);
                if fwd.len() > 100 {
                    fwd.remove(0);
                }
            }
        }
        return play_track_inner(&state, prev, None, false).await;
    }

    let prev_item = {
        let player = state.player.read().await;
        if player.queue.is_empty() {
            return Ok(player.clone());
        }
        let curr_idx = player
            .current_track
            .as_ref()
            .and_then(|curr| player.queue.iter().position(|t| t.id == curr.id));

        let prev_idx = match curr_idx {
            Some(i) => {
                if i == 0 {
                    player.queue.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        player.queue[prev_idx].clone()
    };
    play_track_inner(&state, prev_item, None, false).await
}

#[tauri::command]
pub async fn seek_to(state: State<'_, AppState>, position_sec: u32) -> Result<PlayerState, String> {
    let mut player = state.player.write().await;
    if let Some((duration, source)) = player.current_track.as_ref().map(|t| (t.duration, t.source)) {
        player.position_sec = position_sec.min(duration);
        if source == SourceKind::Spotify {
            let registry = state.registry.read().await;
            if registry.spotify.auth_status().connected {
                let ms = player.position_sec * 1000;
                let _ = registry
                    .spotify
                    .connect_command("PUT", &format!("seek?position_ms={}", ms))
                    .await;
            }
        }
    }
    let snapshot = player.clone();
    state.discord_rpc.update_from_player(&snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, volume: u8) -> Result<PlayerState, String> {
    let clamped = volume.min(100);
    let (snapshot, is_spotify) = {
        let mut player = state.player.write().await;
        player.volume = clamped;
        let sp = player
            .current_track
            .as_ref()
            .is_some_and(|t| t.source == SourceKind::Spotify);
        (player.clone(), sp)
    };
    {
        let mut vault = state.vault.write().await;
        vault.data.volume = Some(clamped);
        let _ = vault.save();
    }
    if is_spotify {
        let registry = state.registry.read().await;
        if registry.spotify.auth_status().connected {
            let _ = registry
                .spotify
                .connect_command("PUT", &format!("volume?volume_percent={}", clamped))
                .await;
        }
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn toggle_shuffle(state: State<'_, AppState>) -> Result<PlayerState, String> {
    let snapshot = {
        let mut player = state.player.write().await;
        player.shuffle = !player.shuffle;
        let wave_on = *state.wave_active.read().await;
        if player.shuffle && !wave_on && player.queue.len() > 1 {
            let hist = state.play_history.read().await.clone();
            let fresh = build_shuffle_bag(
                &player.queue,
                player.current_track.as_ref(),
                &hist,
                player.shuffle_mode,
            );
            player.shuffle_order = fresh.iter().rev().cloned().collect();
            *state.shuffle_bag.write().await = fresh;
        } else {
            player.shuffle_order.clear();
            state.shuffle_bag.write().await.clear();
        }
        player.clone()
    };
    {
        let mut vault = state.vault.write().await;
        vault.data.shuffle = Some(snapshot.shuffle);
        let _ = vault.save();
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn set_shuffle_mode(
    state: State<'_, AppState>,
    mode: ShuffleMode,
) -> Result<PlayerState, String> {
    let snapshot = {
        let mut player = state.player.write().await;
        player.shuffle_mode = mode;
        let wave_on = *state.wave_active.read().await;
        if player.shuffle && !wave_on && player.queue.len() > 1 {
            let hist = state.play_history.read().await.clone();
            let fresh = build_shuffle_bag(
                &player.queue,
                player.current_track.as_ref(),
                &hist,
                mode,
            );
            player.shuffle_order = fresh.iter().rev().cloned().collect();
            *state.shuffle_bag.write().await = fresh;
        }
        player.clone()
    };
    {
        let mut vault = state.vault.write().await;
        vault.data.shuffle_mode = Some(mode);
        let _ = vault.save();
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn cycle_repeat(state: State<'_, AppState>) -> Result<PlayerState, String> {
    let snapshot = {
        let mut player = state.player.write().await;
        player.repeat = match player.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        player.clone()
    };
    {
        let mut vault = state.vault.write().await;
        vault.data.repeat = Some(snapshot.repeat);
        let _ = vault.save();
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_sources_status(state: State<'_, AppState>) -> Result<Vec<SourceStatus>, String> {
    let registry = state.registry.read().await;
    Ok(registry.all_statuses())
}

#[tauri::command]
pub async fn update_source_proxy(
    state: State<'_, AppState>,
    source: SourceKind,
    config: ProxyConfig,
) -> Result<Vec<SourceStatus>, String> {
    {
        let mut vault = state.vault.write().await;
        vault.data.proxies.insert(source, config.clone());
        let _ = vault.save();
    }
    let mut registry = state.registry.write().await;
    registry
        .update_proxy(source, config)
        .map_err(|e| e.to_string())?;
    Ok(registry.all_statuses())
}

#[tauri::command]
pub async fn test_source_connection(
    state: State<'_, AppState>,
    source: SourceKind,
) -> Result<SourceStatus, String> {
    let mut registry = state.registry.write().await;
    Ok(registry.test_connection(source).await)
}

#[tauri::command]
pub async fn connect_yandex(
    state: State<'_, AppState>,
    token: String,
) -> Result<Vec<SourceStatus>, String> {
    let raw = token.trim();
    // Если пользователь скопировал всю ссылку из адресной строки после OAuth
    let clean_token = if let Some(pos) = raw.find("access_token=") {
        let after = &raw[pos + "access_token=".len()..];
        after.split(&['&', '#'][..]).next().unwrap_or("").trim().to_string()
    } else {
        raw.to_string()
    };

    if clean_token.is_empty() {
        return Err("Вставьте OAuth-токен или скопированную ссылку после авторизации".into());
    }

    let account = {
        let mut registry = state.registry.write().await;
        let (name, _uid) = registry
            .yandex
            .verify_and_fetch_account(&clean_token)
            .await
            .map_err(|e| e.to_string())?;
        name
    };

    {
        let mut vault = state.vault.write().await;
        vault.data.yandex_token = Some(clean_token);
        vault.data.yandex_account = Some(account);
        vault.save()?;
    }

    let registry = state.registry.read().await;
    Ok(registry.all_statuses())
}

#[tauri::command]
pub async fn connect_soundcloud(
    state: State<'_, AppState>,
    oauth_token: Option<String>,
    client_id: Option<String>,
) -> Result<Vec<SourceStatus>, String> {
    let tok = oauth_token.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let cid = client_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let account = {
        let mut registry = state.registry.write().await;
        registry
            .soundcloud
            .verify_credentials(tok.clone(), cid.clone())
            .await
            .map_err(|e| e.to_string())?
    };

    {
        let mut vault = state.vault.write().await;
        vault.data.soundcloud_token = tok;
        vault.data.soundcloud_client_id = cid;
        vault.data.soundcloud_account = Some(account);
        vault.save()?;
    }

    let registry = state.registry.read().await;
    Ok(registry.all_statuses())
}

#[tauri::command]
pub async fn start_spotify_oauth(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: String,
) -> Result<Vec<SourceStatus>, String> {
    let cid = client_id.trim().to_string();
    if cid.is_empty() {
        return Err("Укажите Spotify Client ID из Developer Dashboard".into());
    }

    let (verifier, challenge) = generate_pkce_pair();
    let redirect_uri = "http://127.0.0.1:8888/callback";
    let scope = "user-read-private user-library-read user-read-recently-played playlist-read-private user-modify-playback-state user-read-playback-state";

    let mut auth_url = url::Url::parse("https://accounts.spotify.com/authorize").unwrap();
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &cid)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge)
        .append_pair("scope", scope);

    let listener = TcpListener::bind("127.0.0.1:8888")
        .await
        .map_err(|e| format!("Порт 8888 занят: {}", e))?;

    open_in_browser(auth_url.as_str())?;

    let auth_code = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        loop {
            let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                continue;
            }
            let req_str = String::from_utf8_lossy(&buf[..n]);
            let first_line = req_str.lines().next().unwrap_or("");
            let path_part = first_line.split_whitespace().nth(1).unwrap_or("/");

            if !path_part.starts_with("/callback") {
                let _ = socket
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
                continue;
            }

            let parsed = url::Url::parse(&format!("http://127.0.0.1:8888{}", path_part))
                .map_err(|e| e.to_string())?;

            let code = parsed
                .query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.to_string());
            let err_param = parsed
                .query_pairs()
                .find(|(k, _)| k == "error")
                .map(|(_, v)| v.to_string());

            let html = if code.is_some() {
                "<html><body style='background:#111215;color:#f4f4f7;font-family:sans-serif;display:grid;place-items:center;height:100vh'><div><h2>Spotify подключен</h2><p>Можете закрыть эту вкладку и вернуться в Flowly.</p></div></body></html>".to_string()
            } else {
                let reason = err_param.clone().unwrap_or_else(|| "Код доступа не получен".into());
                format!(
                    "<html><body style='background:#111215;color:#fb7185;font-family:sans-serif;display:grid;place-items:center;height:100vh'><div><h2>Ошибка авторизации Spotify</h2><p>{}</p></div></body></html>",
                    reason
                )
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = socket.write_all(response.as_bytes()).await;

            if let Some(c) = code {
                return Ok::<String, String>(c);
            }
            return Err(format!(
                "Spotify отклонил авторизацию: {}",
                err_param.unwrap_or_else(|| "отменено пользователем".into())
            ));
        }
    })
    .await
    .map_err(|_| "Время ожидания авторизации Spotify истекло (120 сек)".to_string())??;

    let client = {
        let registry = state.registry.read().await;
        crate::providers::MusicProvider::client(&registry.spotify).clone()
    };

    let token_resp = client
        .post("https://accounts.spotify.com/api/token")
        .form(&[
            ("client_id", cid.as_str()),
            ("grant_type", "authorization_code"),
            ("code", auth_code.as_str()),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| {
            format!(
                "Не удалось связаться с accounts.spotify.com ({}). Проверьте прокси в блоке «Сеть» (если у вас системный VPN/TUN — выберите Direct, если локальный прокси — проверьте порт).",
                e
            )
        })?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Ошибка обмена токена Spotify ({}): {}", status, body));
    }

    let token_json: Value = token_resp.json().await.map_err(|e| e.to_string())?;
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or_else(|| "Не получен access_token Spotify".to_string())?
        .to_string();
    let refresh_token = token_json["refresh_token"].as_str().map(String::from);

    let account = {
        let mut registry = state.registry.write().await;
        match registry.spotify.verify_and_fetch_account(&access_token).await {
            Ok(name) => name,
            Err(_) => {
                // Повторная попытка при временном сетевом сбое
                match registry.spotify.verify_and_fetch_account(&access_token).await {
                    Ok(name) => name,
                    Err(_) => {
                        let fallback_name = "Spotify User".to_string();
                        registry
                            .spotify
                            .set_credentials(Some(access_token.clone()), Some(fallback_name.clone()));
                        fallback_name
                    }
                }
            }
        }
    };

    {
        let mut vault = state.vault.write().await;
        vault.data.spotify_client_id = Some(cid);
        vault.data.spotify_access_token = Some(access_token);
        vault.data.spotify_refresh_token = refresh_token;
        vault.data.spotify_account = Some(account);
        vault.save()?;
    }

    let statuses = {
        let registry = state.registry.read().await;
        registry.all_statuses()
    };
    let _ = app.emit("auth-updated", &statuses);
    Ok(statuses)
}

#[tauri::command]
pub async fn disconnect_source(
    state: State<'_, AppState>,
    source: SourceKind,
) -> Result<Vec<SourceStatus>, String> {
    {
        let mut vault = state.vault.write().await;
        match source {
            SourceKind::Yandex => {
                vault.data.yandex_token = None;
                vault.data.yandex_account = None;
            }
            SourceKind::Spotify => {
                vault.data.spotify_access_token = None;
                vault.data.spotify_refresh_token = None;
                vault.data.spotify_account = None;
            }
            SourceKind::SoundCloud => {
                vault.data.soundcloud_token = None;
                vault.data.soundcloud_client_id = None;
                vault.data.soundcloud_account = None;
            }
            SourceKind::Local => {}
        }
        vault.save()?;
    }

    let mut registry = state.registry.write().await;
    match source {
        SourceKind::Yandex => registry.yandex.set_credentials(None, None, None),
        SourceKind::Spotify => registry.spotify.set_credentials(None, None),
        SourceKind::SoundCloud => registry.soundcloud.set_credentials(None, None, None),
        SourceKind::Local => {}
    }

    Ok(registry.all_statuses())
}

#[tauri::command]
pub async fn get_all_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    let local = {
        let vault = state.vault.read().await;
        vault.data.local_playlists.clone()
    };
    let mut remote = {
        let mut registry = state.registry.write().await;
        registry.fetch_remote_playlists().await
    };
    let mut all = local;
    all.append(&mut remote);
    Ok(all)
}

#[tauri::command]
pub async fn create_local_playlist(
    state: State<'_, AppState>,
    title: String,
    description: Option<String>,
) -> Result<Vec<Playlist>, String> {
    let clean = title.trim().to_string();
    if clean.is_empty() {
        return Err("Введите название плейлиста".into());
    }
    {
        let mut vault = state.vault.write().await;
        let id = format!(
            "local-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        vault.data.local_playlists.push(Playlist {
            id,
            title: clean,
            description,
            source: None,
            is_local: true,
            artwork: None,
            tracks: Vec::new(),
        });
        vault.save()?;
    }
    get_all_playlists(state).await
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    track: Track,
) -> Result<Vec<Playlist>, String> {
    {
        let mut vault = state.vault.write().await;
        if let Some(pl) = vault
            .data
            .local_playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
        {
            if !pl.tracks.iter().any(|t| t.id == track.id) {
                if pl.artwork.is_none() {
                    pl.artwork = Some(track.artwork.clone());
                }
                pl.tracks.push(track);
            }
            vault.save()?;
        }
    }
    get_all_playlists(state).await
}

#[tauri::command]
pub async fn remove_track_from_playlist(
    state: State<'_, AppState>,
    playlist_id: String,
    track_id: String,
) -> Result<Vec<Playlist>, String> {
    {
        let mut vault = state.vault.write().await;
        if let Some(pl) = vault
            .data
            .local_playlists
            .iter_mut()
            .find(|p| p.id == playlist_id)
        {
            pl.tracks.retain(|t| t.id != track_id);
            vault.save()?;
        }
    }
    get_all_playlists(state).await
}

#[tauri::command]
pub async fn get_audio_effects(state: State<'_, AppState>) -> Result<AudioEffectsConfig, String> {
    let effects = state.effects.read().await;
    Ok(effects.clone())
}

#[tauri::command]
pub async fn update_audio_effects(
    state: State<'_, AppState>,
    config: AudioEffectsConfig,
) -> Result<AudioEffectsConfig, String> {
    {
        let mut vault = state.vault.write().await;
        vault.data.effects = Some(config.clone());
        let _ = vault.save();
    }
    let mut effects = state.effects.write().await;
    *effects = config;
    Ok(effects.clone())
}

#[tauri::command]
pub async fn get_liked_ids(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let vault = state.vault.read().await;
    Ok(vault.data.liked_ids.iter().cloned().collect())
}

#[tauri::command]
pub async fn toggle_like_track(
    state: State<'_, AppState>,
    track: Track,
    liked: bool,
) -> Result<Vec<String>, String> {
    {
        let mut registry = state.registry.write().await;
        match track.source {
            SourceKind::Yandex => {
                if registry.yandex.auth_status().connected {
                    let _ = registry.yandex.set_track_like(&track.id, liked).await;
                }
            }
            SourceKind::Spotify => {
                if registry.spotify.auth_status().connected {
                    let _ = registry.spotify.set_track_like(&track.id, liked).await;
                }
            }
            SourceKind::SoundCloud => {
                if registry.soundcloud.auth_status().connected {
                    let _ = registry.soundcloud.set_track_like(&track.id, liked).await;
                }
            }
            SourceKind::Local => {}
        }
    }

    let mut vault = state.vault.write().await;
    let tid = track.id.clone();
    if liked {
        vault.data.liked_ids.insert(tid.clone());
        vault.data.liked_tracks.retain(|t| t.id != tid);
        vault.data.liked_tracks.insert(0, track);
    } else {
        vault.data.liked_ids.remove(&tid);
        vault.data.liked_tracks.retain(|t| t.id != tid);
    }
    let _ = vault.save();
    Ok(vault.data.liked_ids.iter().cloned().collect())
}

#[tauri::command]
pub async fn get_disliked_ids(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let vault = state.vault.read().await;
    Ok(vault.data.disliked_ids.iter().cloned().collect())
}

#[tauri::command]
pub async fn dislike_track(state: State<'_, AppState>, track: Track) -> Result<PlayerState, String> {
    if track.source == SourceKind::Yandex {
        let mut registry = state.registry.write().await;
        if registry.yandex.auth_status().connected {
            let _ = registry.yandex.set_track_dislike(&track.id).await;
        }
    }

    let tid = track.id.clone();
    {
        let mut vault = state.vault.write().await;
        vault.data.disliked_ids.insert(tid.clone());
        vault.data.liked_ids.remove(&tid);
        vault.data.liked_tracks.retain(|t| t.id != tid);
        let _ = vault.save();
    }

    state.forward_history.write().await.retain(|t| t.id != tid);
    state.play_history.write().await.retain(|t| t.id != tid);

    let was_current = {
        let mut player = state.player.write().await;
        let is_cur = player.current_track.as_ref().is_some_and(|c| c.id == tid);
        player.queue.retain(|t| t.id != tid);
        {
            let mut bag = state.shuffle_bag.write().await;
            bag.retain(|id| id != &tid);
            player.shuffle_order = bag.iter().rev().cloned().collect();
        }
        is_cur
    };

    if was_current {
        next_track(state).await
    } else {
        let p = state.player.read().await.clone();
        Ok(p)
    }
}

#[tauri::command]
pub async fn start_my_wave(
    state: State<'_, AppState>,
    source_filter: Option<SourceKind>,
) -> Result<PlayerState, String> {
    let exclude_ids = {
        let vault = state.vault.read().await;
        let mut ex = vault.data.disliked_ids.clone();
        for h in state.play_history.read().await.iter().rev().take(25) {
            ex.insert(h.id.clone());
        }
        ex
    };
    let wave_tracks = {
        let mut registry = state.registry.write().await;
        registry
            .generate_wave_queue_excluding(source_filter, &exclude_ids, None)
            .await
    };
    let Some(first) = wave_tracks.first().cloned() else {
        return Err("Не удалось подобрать треки для волны — подключите хотя бы один сервис".into());
    };
    let res = play_track_inner(&state, first, Some(wave_tracks), true).await?;
    {
        let mut w = state.wave_active.write().await;
        *w = true;
    }
    {
        let mut ws = state.wave_source.write().await;
        *ws = source_filter;
    }
    Ok(res)
}

#[tauri::command]
pub async fn reset_wave_to_library(
    state: State<'_, AppState>,
    source_filter: Option<SourceKind>,
) -> Result<PlayerState, String> {
    *state.wave_active.write().await = false;
    *state.wave_source.write().await = None;

    let mut lib_tracks = get_library_section(state.clone(), LibrarySection::AllTracks, source_filter).await?;
    let disliked = { state.vault.read().await.data.disliked_ids.clone() };
    lib_tracks.retain(|t| !disliked.contains(&t.id));

    if lib_tracks.is_empty() {
        return Err("Библиотека пуста — не удалось выбрать случайный трек".into());
    }

    {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        lib_tracks.shuffle(&mut rng);
    }
    let first = lib_tracks[0].clone();
    play_track_inner(&state, first, Some(lib_tracks), true).await
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkLyricsSettings {
    pub doh_enabled: bool,
    pub doh_provider: String,
    pub spicy_lyrics_key: String,
    #[serde(default)]
    pub discord_rpc_enabled: bool,
    #[serde(default)]
    pub discord_client_id: String,
    #[serde(default)]
    pub close_to_tray: bool,
    #[serde(default)]
    pub dynamic_theme_enabled: bool,
    #[serde(default)]
    pub weather_city: String,
}

#[tauri::command]
pub async fn get_network_lyrics_settings(
    state: State<'_, AppState>,
) -> Result<NetworkLyricsSettings, String> {
    let vault = state.vault.read().await;
    Ok(NetworkLyricsSettings {
        doh_enabled: vault.data.doh_enabled.unwrap_or(true),
        doh_provider: vault
            .data
            .doh_provider
            .clone()
            .unwrap_or_else(|| "cloudflare".to_string()),
        spicy_lyrics_key: vault.data.spicy_lyrics_key.clone().unwrap_or_default(),
        discord_rpc_enabled: vault.data.discord_rpc_enabled.unwrap_or(true),
        discord_client_id: vault.data.discord_client_id.clone().unwrap_or_default(),
        close_to_tray: vault.data.close_to_tray.unwrap_or(false),
        dynamic_theme_enabled: vault.data.dynamic_theme_enabled.unwrap_or(false),
        weather_city: vault.data.weather_city.clone().unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn update_network_lyrics_settings(
    state: State<'_, AppState>,
    settings: NetworkLyricsSettings,
) -> Result<NetworkLyricsSettings, String> {
    crate::network::doh::set_doh_config(settings.doh_enabled, &settings.doh_provider);
    let clean_dc_id = settings.discord_client_id.trim().to_string();
    state
        .discord_rpc
        .configure(settings.discord_rpc_enabled, clean_dc_id.clone());
    if settings.discord_rpc_enabled {
        let p = state.player.read().await.clone();
        state.discord_rpc.update_from_player(&p);
    }
    {
        let mut vault = state.vault.write().await;
        vault.data.doh_enabled = Some(settings.doh_enabled);
        vault.data.doh_provider = Some(settings.doh_provider.clone());
        vault.data.spicy_lyrics_key = if settings.spicy_lyrics_key.trim().is_empty() {
            None
        } else {
            Some(settings.spicy_lyrics_key.trim().to_string())
        };
        vault.data.discord_rpc_enabled = Some(settings.discord_rpc_enabled);
        vault.data.discord_client_id = if clean_dc_id.is_empty() {
            None
        } else {
            Some(clean_dc_id)
        };
        vault.data.close_to_tray = Some(settings.close_to_tray);
        vault.data.dynamic_theme_enabled = Some(settings.dynamic_theme_enabled);
        let clean_city = settings.weather_city.trim().to_string();
        vault.data.weather_city = if clean_city.is_empty() {
            None
        } else {
            Some(clean_city)
        };
        let _ = vault.save();
    }
    get_network_lyrics_settings(state).await
}

#[tauri::command]
pub async fn get_artist_yandex_info(
    state: State<'_, AppState>,
    artist_name: String,
) -> Result<Option<crate::models::ArtistYandexInfo>, String> {
    let reg = state.registry.read().await;
    reg.yandex
        .fetch_artist_info(&artist_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_yandex_album_tracks(
    state: State<'_, AppState>,
    album_id: String,
) -> Result<Vec<Track>, String> {
    let reg = state.registry.read().await;
    reg.yandex
        .fetch_album_tracks(&album_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn record_listen(
    state: State<'_, AppState>,
    track: Track,
    listened_sec: u32,
) -> Result<(), String> {
    let mut recap = state.recap.write().await;
    recap.record(track, listened_sec);
    Ok(())
}

#[tauri::command]
pub async fn get_listening_recap(
    state: State<'_, AppState>,
    period: crate::recap::RecapPeriod,
    tz_offset_min: i32,
) -> Result<crate::recap::ListeningRecap, String> {
    let recap = state.recap.read().await;
    Ok(recap.build_recap(period, tz_offset_min))
}

#[tauri::command]
pub async fn get_track_samples(
    artist: String,
    title: String,
) -> Result<crate::samples::TrackSamplesResponse, String> {
    Ok(crate::samples::fetch_track_samples(&artist, &title).await)
}

#[tauri::command]
pub async fn preload_next_track(state: State<'_, AppState>) -> Result<Option<Track>, String> {
    let candidate = {
        let player = state.player.read().await;
        if player.queue.len() <= 1 {
            return Ok(None);
        }
        let wave_on = *state.wave_active.read().await;
        let curr_id = player.current_track.as_ref().map(|c| c.id.as_str());
        if player.shuffle && !wave_on {
            player
                .shuffle_order
                .first()
                .and_then(|id| player.queue.iter().find(|t| &t.id == id).cloned())
        } else {
            let curr_idx = curr_id.and_then(|id| player.queue.iter().position(|t| t.id == id));
            curr_idx.map(|i| player.queue[(i + 1) % player.queue.len()].clone())
        }
    };

    let Some(mut track) = candidate else {
        return Ok(None);
    };
    if track.source != SourceKind::Spotify && track.source != SourceKind::Local {
        let registry = state.registry.read().await;
        if let Ok(Some(url)) = registry.resolve_playback_url(track.source, &track.id).await {
            track.playback_url = Some(url);
        }
    }
    Ok(Some(track))
}

#[tauri::command]
pub async fn generate_radio_host_line(
    state: State<'_, AppState>,
    track: Track,
    engine: String,
    voice: String,
    city: String,
    local_hour: u8,
    is_wave: bool,
) -> Result<crate::radio_host::RadioHostResult, String> {
    let clean_city = city.trim().to_string();
    let weather = if !clean_city.is_empty() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let cached = {
            let guard = state.weather_cache.read().await;
            guard.as_ref().and_then(|c| {
                if c.city.eq_ignore_ascii_case(&clean_city) && now.saturating_sub(c.fetched_at) < 1800 {
                    Some(c.summary.clone())
                } else {
                    None
                }
            })
        };
        if let Some(w) = cached {
            Some(w)
        } else if let Some(fresh) = crate::radio_host::fetch_weather_summary(&clean_city).await {
            *state.weather_cache.write().await = Some(crate::radio_host::CachedWeather {
                city: clean_city,
                summary: fresh.clone(),
                fetched_at: now,
            });
            Some(fresh)
        } else {
            None
        }
    } else {
        None
    };

    let text = crate::radio_host::build_radio_phrase(&track, local_hour, weather.as_deref(), is_wave);

    let (audio_data_url, used_engine, error) = match engine.as_str() {
        "piper" => match crate::radio_host::synthesize_piper_tts(&state.app_data_dir, &text, &voice).await {
            Ok(url) => (Some(url), "piper".to_string(), None),
            Err(e) => (None, "system".to_string(), Some(e)),
        },
        "system" => (None, "system".to_string(), None),
        _ => match crate::radio_host::synthesize_edge_tts(&text, &voice).await {
            Ok(url) => (Some(url), "edge".to_string(), None),
            Err(_) => match crate::radio_host::synthesize_google_tts(&text).await {
                Ok(url) => (Some(url), "google-neural".to_string(), None),
                Err(e) => (None, "system".to_string(), Some(e)),
            },
        },
    };

    Ok(crate::radio_host::RadioHostResult {
        text,
        audio_data_url,
        used_engine,
        error,
    })
}

#[tauri::command]
pub async fn install_piper_tts(state: State<'_, AppState>) -> Result<String, String> {
    crate::radio_host::install_piper_bundle(&state.app_data_dir).await
}

#[tauri::command]
pub async fn get_dynamic_atmosphere(
    city: String,
) -> Result<Option<crate::radio_host::WeatherAtmosphere>, String> {
    Ok(crate::radio_host::fetch_weather_atmosphere(&city).await)
}

#[tauri::command]
pub async fn get_active_window_info(
    custom_games: Vec<String>,
) -> Result<crate::activity::ActivityInfo, String> {
    Ok(crate::activity::detect_active_window(&custom_games))
}

#[tauri::command]
pub async fn test_doh_server(provider: String) -> Result<u64, String> {
    let resolver = crate::network::doh::DohResolver::new();
    resolver.test_provider(&provider).await
}

#[tauri::command]
pub async fn get_track_lyrics(
    state: State<'_, AppState>,
    track: Track,
) -> Result<LyricsResponse, String> {
    let (client, ym_token, spicy_key) = {
        let registry = state.registry.read().await;
        let vault = state.vault.read().await;
        let resolved_spicy = vault
            .data
            .spicy_lyrics_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::lyrics::DEFAULT_SPICY_LYRICS_KEY)
            .to_string();
        (
            registry.yandex.client().clone(),
            vault.data.yandex_token.clone(),
            Some(resolved_spicy),
        )
    };
    Ok(fetch_lyrics(&client, ym_token.as_deref(), spicy_key.as_deref(), &track).await)
}

#[tauri::command]
pub fn get_audio_proxy_port(state: State<'_, AppState>) -> u16 {
    state.audio_proxy_port
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Недопустимый URL".into());
    }
    open_in_browser(&url)
}

fn open_in_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let escaped = url.replace('\'', "''");
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &format!("Start-Process '{}'", escaped),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_local_folders(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let vault = state.vault.read().await;
    Ok(vault.data.local_folders.clone())
}

#[tauri::command]
pub async fn pick_and_add_local_folder(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Выберите папку с музыкой")
        .pick_folder()
        .await;
    let Some(folder) = picked else {
        let vault = state.vault.read().await;
        return Ok(vault.data.local_folders.clone());
    };
    let path_str = folder.path().to_string_lossy().to_string();
    let mut vault = state.vault.write().await;
    if !vault.data.local_folders.iter().any(|p| p.eq_ignore_ascii_case(&path_str)) {
        vault.data.local_folders.push(path_str);
        vault.save()?;
    }
    Ok(vault.data.local_folders.clone())
}

#[tauri::command]
pub async fn remove_local_folder(state: State<'_, AppState>, path: String) -> Result<Vec<String>, String> {
    let mut vault = state.vault.write().await;
    vault.data.local_folders.retain(|p| p != &path);
    vault.save()?;
    Ok(vault.data.local_folders.clone())
}

#[tauri::command]
pub async fn scan_local_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    let folders = {
        let vault = state.vault.read().await;
        vault.data.local_folders.clone()
    };
    tokio::task::spawn_blocking(move || crate::local_library::scan_local_folders(&folders))
        .await
        .map_err(|e| e.to_string())
}
