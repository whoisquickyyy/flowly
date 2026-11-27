use std::process::Command;

use tauri::State;

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

    if section == LibrarySection::RecentlyPlayed {
        let hist: Vec<Track> = state
            .play_history
            .read()
            .await
            .iter()
            .rev()
            .filter(|t| source_filter.is_none() || source_filter == Some(t.source))
            .cloned()
            .collect();
        let mut merged = Vec::with_capacity(hist.len() + remote.len());
        let mut seen = std::collections::HashSet::<String>::new();
        for t in hist.into_iter().chain(remote.into_iter()) {
            if seen.insert(t.id.clone()) {
                merged.push(t);
            }
        }
        remote = merged;
    } else if matches!(section, LibrarySection::LikedSongs | LibrarySection::AllTracks) {
        let vault = state.vault.read().await;
        let mut combined: Vec<Track> = vault
            .data
            .liked_tracks
            .iter()
            .filter(|t| source_filter.is_none() || source_filter == Some(t.source))
            .cloned()
            .collect();

        // Для раздела «Все треки» добавляем также историю прослушиваний и локальные файлы,
        // чтобы «Все треки» отличались от «Любимых»
        if section == LibrarySection::AllTracks {
            drop(vault);
            let hist = state.play_history.read().await.clone();
            for t in hist.into_iter().rev() {
                if source_filter.is_none() || source_filter == Some(t.source) {
                    combined.push(t);
                }
            }
        }

        if !combined.is_empty() || section == LibrarySection::AllTracks {
            let mut merged = Vec::with_capacity(combined.len() + remote.len());
            let mut seen = std::collections::HashSet::<String>::new();
            for t in combined.into_iter().chain(remote.into_iter()) {
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
    if track.source != SourceKind::Local {
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
    let need_resolve = {
        let player = state.player.read().await;
        if let Some(ref tr) = player.current_track {
            !player.is_playing && tr.source != SourceKind::Local && tr.playback_url.is_none()
        } else {
            false
        }
    };

    if need_resolve {
        let target = { state.player.read().await.current_track.clone() };
        if let Some(tr) = target {
            let registry = state.registry.read().await;
            if let Ok(Some(url)) = registry.resolve_playback_url(tr.source, &tr.id).await {
                let mut player = state.player.write().await;
                if let Some(ref mut cur) = player.current_track {
                    if cur.id == tr.id {
                        cur.playback_url = Some(url);
                    }
                }
            }
        }
    }

    let mut player = state.player.write().await;
    if player.current_track.is_some() {
        player.is_playing = !player.is_playing;
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
        let mut more = {
            let mut registry = state.registry.write().await;
            registry
                .generate_wave_queue_excluding(wave_src, &exclude_ids, ym_seed.as_deref())
                .await
        };
        if more.is_empty() {
            // Если строгий фильтр (вся очередь + вся история) отсеял все треки,
            // ослабляем исключение до дизлайков, текущего трека и последних 5 треков истории,
            // чтобы волна никогда не зацикливалась на 2 треках.
            let mut relaxed = {
                let vault = state.vault.read().await;
                vault.data.disliked_ids.clone()
            };
            {
                let player = state.player.read().await;
                if let Some(curr) = player.current_track.as_ref() {
                    relaxed.insert(curr.id.clone());
                }
            }
            for h in state.play_history.read().await.iter().rev().take(5) {
                relaxed.insert(h.id.clone());
            }
            let mut registry = state.registry.write().await;
            more = registry
                .generate_wave_queue_excluding(wave_src, &relaxed, ym_seed.as_deref())
                .await;
            // Разрешаем добавлять такие треки в хвост очереди, если не совпадают с текущим/недавними
            exclude_ids = relaxed;
        }
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
        // В режиме «Моя волна» шафл отключён — треки идут строго в порядке потока
        if player.shuffle {
            player.shuffle = false;
            player.shuffle_order.clear();
            state.shuffle_bag.write().await.clear();
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
            let need_autopilot = match curr_idx {
                Some(i) => player.repeat == RepeatMode::Off && i + 1 >= player.queue.len(),
                None => false,
            };

            if need_autopilot {
                let ym_seed = player
                    .current_track
                    .as_ref()
                    .filter(|t| t.source == SourceKind::Yandex)
                    .map(|t| t.id.clone())
                    .or_else(|| {
                        player
                            .queue
                            .iter()
                            .rev()
                            .find(|t| t.source == SourceKind::Yandex)
                            .map(|t| t.id.clone())
                    });
                let mut ex: std::collections::HashSet<String> =
                    player.queue.iter().map(|t| t.id.clone()).collect();
                for h in state.play_history.read().await.iter() {
                    ex.insert(h.id.clone());
                }
                {
                    let vault = state.vault.read().await;
                    ex.extend(vault.data.disliked_ids.iter().cloned());
                }
                drop(player);
                let mut more = {
                    let mut registry = state.registry.write().await;
                    registry
                        .generate_wave_queue_excluding(None, &ex, ym_seed.as_deref())
                        .await
                };
                {
                    let weights = state.wave_weights.read().await;
                    if !weights.is_empty() {
                        more.sort_by(|a, b| {
                            wave_track_score(b, &weights)
                                .partial_cmp(&wave_track_score(a, &weights))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                }
                let mut player = state.player.write().await;
                let mut added: Vec<Track> = Vec::new();
                for t in more {
                    if !ex.contains(&t.id) {
                        ex.insert(t.id.clone());
                        added.push(t.clone());
                        player.queue.push(t);
                    }
                }
                if let Some(first_new) = added.into_iter().next() {
                    *state.wave_active.write().await = true;
                    first_new
                } else {
                    player.is_playing = false;
                    return Ok(player.clone());
                }
            } else {
                let next_idx = match curr_idx {
                    Some(i) => (i + 1) % player.queue.len(),
                    None => 0,
                };
                player.queue[next_idx].clone()
            }
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
    if let Some(duration) = player.current_track.as_ref().map(|t| t.duration) {
        player.position_sec = position_sec.min(duration);
    }
    let snapshot = player.clone();
    state.discord_rpc.update_from_player(&snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, volume: u8) -> Result<PlayerState, String> {
    let clamped = volume.min(100);
    let snapshot = {
        let mut player = state.player.write().await;
        player.volume = clamped;
        player.clone()
    };
    {
        let mut vault = state.vault.write().await;
        vault.data.volume = Some(clamped);
        let _ = vault.save();
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn toggle_shuffle(state: State<'_, AppState>) -> Result<PlayerState, String> {
    let wave_on = *state.wave_active.read().await;
    if wave_on {
        // В режиме «Моя волна» перемешивание отключено
        let mut player = state.player.write().await;
        player.shuffle = false;
        player.shuffle_order.clear();
        state.shuffle_bag.write().await.clear();
        return Ok(player.clone());
    }
    let snapshot = {
        let mut player = state.player.write().await;
        player.shuffle = !player.shuffle;
        if player.shuffle && player.queue.len() > 1 {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundCloudPollResult {
    pub status: crate::auth::SoundCloudOAuthStatus,
    pub sources: Option<Vec<SourceStatus>>,
}

fn generate_sc_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn generate_pkce_pair() -> (String, String) {
    use base64::Engine;
    use rand::Rng;
    use sha2::{Digest, Sha256};
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    (verifier, challenge)
}

#[tauri::command]
pub async fn start_soundcloud_oauth(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    relay_url: Option<String>,
) -> Result<crate::auth::SoundCloudOAuthStatus, String> {
    use tauri_plugin_opener::OpenerExt;

    let explicit_relay = relay_url
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty());
    let stored_relay = {
        let vault = state.vault.read().await;
        vault.data.soundcloud_oauth_relay.clone()
    };
    let default_relay = option_env!("FLOWLY_SC_OAUTH_URL")
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://flowly-sc-oauth.demashein.workers.dev".to_string());

    let relay = explicit_relay
        .clone()
        .or(stored_relay)
        .unwrap_or(default_relay);

    if !relay.starts_with("https://") && !relay.starts_with("http://") {
        return Err("Некорректный URL OAuth-ретранслятора (нужен https:// или http://)".to_string());
    }

    if let Some(ref r) = explicit_relay {
        let mut vault = state.vault.write().await;
        vault.data.soundcloud_oauth_relay = Some(r.clone());
        let _ = vault.save();
    }

    let session_id = generate_sc_session_id();
    let (code_verifier, code_challenge) = generate_pkce_pair();

    let auth_url = {
        let mut ser = url::form_urlencoded::Serializer::new(String::new());
        ser.append_pair("session_id", &session_id);
        ser.append_pair("code_challenge", &code_challenge);
        format!("{}/auth/start?{}", relay, ser.finish())
    };

    let status_snapshot = {
        let mut oauth = state.sc_oauth.write().await;
        oauth.session_id = Some(session_id);
        oauth.code_verifier = Some(code_verifier);
        oauth.relay_url = Some(relay);
        oauth.pending = true;
        oauth.authenticated = false;
        oauth.account_name = None;
        oauth.auth_url = Some(auth_url.clone());
        oauth.error = None;
        oauth.to_status()
    };

    app.opener()
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| format!("Не удалось открыть системный браузер: {}", e))?;

    Ok(status_snapshot)
}

#[tauri::command]
pub async fn poll_soundcloud_oauth(
    state: State<'_, AppState>,
) -> Result<SoundCloudPollResult, String> {
    let (pending, session_id, code_verifier, relay_url) = {
        let guard = state.sc_oauth.read().await;
        (
            guard.pending,
            guard.session_id.clone(),
            guard.code_verifier.clone(),
            guard.relay_url.clone(),
        )
    };

    if pending {
        if let (Some(sid), Some(verifier), Some(relay)) = (session_id, code_verifier, relay_url) {
            let poll_url = {
                let mut ser = url::form_urlencoded::Serializer::new(String::new());
                ser.append_pair("session_id", &sid);
                ser.append_pair("code_verifier", &verifier);
                format!("{}/session?{}", relay, ser.finish())
            };

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(6))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            if let Ok(resp) = client.get(&poll_url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    match json["status"].as_str() {
                        Some("ok") => {
                            if let Some(tok) = json["access_token"]
                                .as_str()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(String::from)
                            {
                                let worker_username = json["username"]
                                    .as_str()
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(String::from);

                                let verify_res = {
                                    let mut reg = state.registry.write().await;
                                    match reg
                                        .soundcloud
                                        .verify_credentials(Some(tok.clone()), None)
                                        .await
                                    {
                                        Ok(acc) => Ok(acc),
                                        Err(e) => {
                                            if let Some(uname) = worker_username {
                                                reg.soundcloud.set_credentials(
                                                    Some(tok.clone()),
                                                    None,
                                                    Some(uname.clone()),
                                                );
                                                Ok(uname)
                                            } else {
                                                Err(e)
                                            }
                                        }
                                    }
                                };
                                match verify_res {
                                    Ok(account) => {
                                        {
                                            let mut vault = state.vault.write().await;
                                            vault.data.soundcloud_token = Some(tok);
                                            vault.data.soundcloud_account = Some(account.clone());
                                            vault.data.soundcloud_session_id = Some(sid.clone());
                                            let _ = vault.save();
                                        }
                                        let mut guard = state.sc_oauth.write().await;
                                        guard.pending = false;
                                        guard.authenticated = true;
                                        guard.account_name = Some(account);
                                        guard.code_verifier = None;
                                        guard.error = None;
                                    }
                                    Err(e) => {
                                        let mut guard = state.sc_oauth.write().await;
                                        guard.pending = false;
                                        guard.authenticated = false;
                                        guard.code_verifier = None;
                                        guard.error = Some(e.to_string());
                                    }
                                }
                            }
                        }
                        Some("error") => {
                            let err = json["error"]
                                .as_str()
                                .unwrap_or("Ошибка авторизации SoundCloud")
                                .to_string();
                            let mut guard = state.sc_oauth.write().await;
                            guard.pending = false;
                            guard.code_verifier = None;
                            guard.error = Some(err);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let status = state.sc_oauth.read().await.to_status();
    let sources = if status.authenticated {
        Some(state.registry.read().await.all_statuses())
    } else {
        None
    };
    Ok(SoundCloudPollResult { status, sources })
}

#[tauri::command]
pub async fn cancel_soundcloud_oauth(
    state: State<'_, AppState>,
) -> Result<crate::auth::SoundCloudOAuthStatus, String> {
    let mut oauth = state.sc_oauth.write().await;
    oauth.pending = false;
    oauth.code_verifier = None;
    oauth.auth_url = None;
    oauth.error = None;
    Ok(oauth.to_status())
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

    let sid = generate_sc_session_id();
    {
        let mut vault = state.vault.write().await;
        vault.data.soundcloud_token = tok;
        vault.data.soundcloud_client_id = cid;
        vault.data.soundcloud_account = Some(account.clone());
        vault.data.soundcloud_session_id = Some(sid.clone());
        vault.save()?;
    }
    {
        let mut oauth = state.sc_oauth.write().await;
        oauth.session_id = Some(sid);
        oauth.pending = false;
        oauth.authenticated = true;
        oauth.account_name = Some(account);
        oauth.auth_url = None;
        oauth.error = None;
    }

    let registry = state.registry.read().await;
    Ok(registry.all_statuses())
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
            SourceKind::SoundCloud => {
                vault.data.soundcloud_token = None;
                vault.data.soundcloud_client_id = None;
                vault.data.soundcloud_account = None;
                vault.data.soundcloud_session_id = None;
            }
            SourceKind::Local => {}
        }
        vault.save()?;
    }

    if source == SourceKind::SoundCloud {
        let mut oauth = state.sc_oauth.write().await;
        oauth.session_id = None;
        oauth.pending = false;
        oauth.authenticated = false;
        oauth.account_name = None;
        oauth.auth_url = None;
        oauth.error = None;
    }

    let mut registry = state.registry.write().await;
    match source {
        SourceKind::Yandex => registry.yandex.set_credentials(None, None, None),
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
    {
        let mut player = state.player.write().await;
        player.shuffle = false;
        player.shuffle_order.clear();
    }
    state.shuffle_bag.write().await.clear();
    let mut res = play_track_inner(&state, first, Some(wave_tracks), true).await?;
    res.shuffle = false;
    res.shuffle_order.clear();
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
pub async fn get_artist_preview_track(
    state: State<'_, AppState>,
    artist_id: String,
    artist_name: String,
) -> Result<Option<Track>, String> {
    let reg = state.registry.read().await;
    reg.yandex
        .fetch_artist_preview_track(&artist_id, &artist_name)
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
    if track.source != SourceKind::Local {
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
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        fetch_lyrics(&client, ym_token.as_deref(), spicy_key.as_deref(), &track),
    )
    .await
    .unwrap_or(LyricsResponse {
        synced: false,
        source: "Таймаут".into(),
        lines: Vec::new(),
    });
    Ok(res)
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

#[tauri::command]
pub fn update_smtc_state(
    smtc: State<'_, crate::media_controls::SmtcHandle>,
    title: String,
    artist: String,
    album: String,
    cover_url: Option<String>,
    duration_sec: u32,
    playing: bool,
    position_sec: u32,
) {
    smtc.send(crate::media_controls::SmtcCommand::Metadata {
        title,
        artist,
        album,
        cover_url,
        duration_sec,
    });
    smtc.send(crate::media_controls::SmtcCommand::Playback {
        playing,
        position_sec,
    });
}

fn wave_track_score(t: &Track, weights: &std::collections::HashMap<String, f32>) -> f32 {
    let art = t
        .artist
        .split(&[',', '&'][..])
        .next()
        .unwrap_or(&t.artist)
        .trim()
        .to_lowercase();
    let mut score = *weights.get(&format!("a:{}", art)).unwrap_or(&0.0);
    if let Some(ref g) = t.genre {
        let gl = g.trim().to_lowercase();
        if !gl.is_empty() {
            score += *weights.get(&format!("g:{}", gl)).unwrap_or(&0.0);
        }
    }
    score
}

#[tauri::command]
pub async fn report_wave_feedback(
    state: State<'_, AppState>,
    track: Track,
    positive: bool,
) -> Result<(), String> {
    let art = track
        .artist
        .split(&[',', '&'][..])
        .next()
        .unwrap_or(&track.artist)
        .trim()
        .to_lowercase();
    let mut weights = state.wave_weights.write().await;
    if !art.is_empty() {
        let k = format!("a:{}", art);
        let cur = *weights.get(&k).unwrap_or(&0.0);
        let delta = if positive { 1.5 } else { -2.5 };
        weights.insert(k, (cur + delta).clamp(-6.0, 6.0));
    }
    if let Some(ref g) = track.genre {
        let gl = g.trim().to_lowercase();
        if !gl.is_empty() {
            let k = format!("g:{}", gl);
            let cur = *weights.get(&k).unwrap_or(&0.0);
            let delta = if positive { 1.0 } else { -1.0 };
            weights.insert(k, (cur + delta).clamp(-4.0, 4.0));
        }
    }
    let snapshot_weights = weights.clone();
    drop(weights);

    if *state.wave_active.read().await {
        let mut player = state.player.write().await;
        if let Some(curr_id) = player.current_track.as_ref().map(|c| c.id.clone()) {
            if let Some(pos) = player.queue.iter().position(|t| t.id == curr_id) {
                if pos + 2 < player.queue.len() {
                    player.queue[pos + 1..].sort_by(|a, b| {
                        wave_track_score(b, &snapshot_weights)
                            .partial_cmp(&wave_track_score(a, &snapshot_weights))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_track_cache_enabled(state: State<'_, AppState>, enabled: bool) {
    state.track_cache.set_enabled(enabled);
}

fn with_kithara<F, R>(state: &State<'_, AppState>, f: F) -> Result<R, String>
where
    F: FnOnce(&mut crate::audio::KitharaEngine) -> Result<R, crate::audio::EngineError>,
{
    let mut guard = state
        .audio_engine
        .lock()
        .map_err(|_| "Audio engine mutex poisoned".to_string())?;
    if guard.is_none() {
        *guard = crate::audio::KitharaEngine::new().ok();
    }
    let engine = guard
        .as_mut()
        .ok_or_else(|| "Kithara audio engine unavailable".to_string())?;
    f(engine).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn audio_load_and_play(state: State<'_, AppState>, url: String) -> Result<(), String> {
    let eng_arc = state.audio_engine.clone();
    tokio::task::spawn_blocking(move || {
        let mut guard = eng_arc
            .lock()
            .map_err(|_| "Audio engine mutex poisoned".to_string())?;
        if guard.is_none() {
            *guard = crate::audio::KitharaEngine::new().ok();
        }
        let engine = guard
            .as_mut()
            .ok_or_else(|| "Kithara audio engine unavailable".to_string())?;
        engine.load_and_play(&url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn audio_prepare_next(state: State<'_, AppState>, url: String) -> Result<(), String> {
    let eng_arc = state.audio_engine.clone();
    tokio::task::spawn_blocking(move || {
        let mut guard = eng_arc
            .lock()
            .map_err(|_| "Audio engine mutex poisoned".to_string())?;
        if guard.is_none() {
            *guard = crate::audio::KitharaEngine::new().ok();
        }
        let engine = guard
            .as_mut()
            .ok_or_else(|| "Kithara audio engine unavailable".to_string())?;
        engine.prepare_next(&url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn audio_crossfade(
    state: State<'_, AppState>,
    duration_secs: f32,
    curve: crate::audio::CrossfadeCurve,
    preset: crate::audio::MixPreset,
    bpm_a: Option<f32>,
    bpm_b: Option<f32>,
    key_a: Option<String>,
    key_b: Option<String>,
) -> Result<(), String> {
    with_kithara(&state, |e| {
        e.crossfade_with_preset(
            duration_secs,
            curve,
            preset,
            bpm_a,
            bpm_b,
            key_a.as_deref(),
            key_b.as_deref(),
        )
    })
}

#[tauri::command]
pub fn audio_pause(state: State<'_, AppState>) -> Result<(), String> {
    with_kithara(&state, |e| e.pause())
}

#[tauri::command]
pub fn audio_resume(state: State<'_, AppState>) -> Result<(), String> {
    with_kithara(&state, |e| e.resume())
}

#[tauri::command]
pub fn audio_seek(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
    with_kithara(&state, |e| e.seek(seconds))
}

#[tauri::command]
pub fn audio_set_volume(state: State<'_, AppState>, volume: f32) -> Result<(), String> {
    with_kithara(&state, |e| {
        e.set_volume(volume);
        Ok(())
    })
}

#[tauri::command]
pub fn audio_set_ducking(state: State<'_, AppState>, ducked: bool) -> Result<(), String> {
    with_kithara(&state, |e| e.set_ducking(ducked))
}

#[tauri::command]
pub fn audio_set_eq_bands(state: State<'_, AppState>, bands: Vec<f32>) -> Result<(), String> {
    with_kithara(&state, |e| e.sync_eq_bands(&bands))
}

#[tauri::command]
pub fn audio_get_status(
    state: State<'_, AppState>,
) -> Result<crate::audio::AudioPlaybackStatus, String> {
    with_kithara(&state, |e| Ok(e.status()))
}
