mod activity;
mod auth;
mod commands;
mod discord_rpc;
mod local_library;
mod lyrics;
mod media_controls;
mod models;
mod network;
mod player;
mod providers;
mod proxy_server;
mod radio_host;
mod recap;
mod samples;
mod track_cache;

use std::sync::Arc;
use player::AppState;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("aura-music"));
            let (listener, proxy_port) = proxy_server::bind_proxy_listener()
                .expect("failed to bind local audio proxy");
            let state = AppState::new(data_dir, proxy_port);
            let registry = Arc::clone(&state.registry);
            let track_cache = state.track_cache.clone();
            tauri::async_runtime::spawn(proxy_server::run_proxy_server(listener, Arc::clone(&registry), track_cache));
            app.manage(media_controls::SmtcHandle::spawn(app.handle()));

            app.manage(state);

            let show_item = MenuItemBuilder::with_id("show", "Открыть Flowly").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Выйти").build(app)?;
            let tray_menu = MenuBuilder::new(app)
                .items(&[&show_item, &quit_item])
                .build()?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&tray_menu)
                .tooltip("Flowly")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.unminimize();
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.unminimize();
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            let _ = tray_builder.build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let close_to_tray = state.vault.blocking_read().data.close_to_tray.unwrap_or(false);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .on_page_load(|window, _| {
            let _ = window.show();
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_library_section,
            commands::search_grouped,
            commands::get_player_state,
            commands::play_track,
            commands::get_liked_ids,
            commands::toggle_like_track,
            commands::get_disliked_ids,
            commands::dislike_track,
            commands::start_my_wave,
            commands::reset_wave_to_library,
            commands::toggle_playback,
            commands::next_track,
            commands::prev_track,
            commands::seek_to,
            commands::set_volume,
            commands::toggle_shuffle,
            commands::set_shuffle_mode,
            commands::cycle_repeat,
            commands::get_sources_status,
            commands::update_source_proxy,
            commands::test_source_connection,
            commands::connect_yandex,
            commands::connect_soundcloud,
            commands::disconnect_source,
            commands::get_all_playlists,
            commands::create_local_playlist,
            commands::add_track_to_playlist,
            commands::remove_track_from_playlist,
            commands::get_audio_effects,
            commands::update_audio_effects,
            commands::get_network_lyrics_settings,
            commands::update_network_lyrics_settings,
            commands::test_doh_server,
            commands::get_track_lyrics,
            commands::get_artist_yandex_info,
            commands::get_artist_preview_track,
            commands::get_yandex_album_tracks,
            commands::record_listen,
            commands::get_listening_recap,
            commands::get_track_samples,
            commands::preload_next_track,
            commands::generate_radio_host_line,
            commands::install_piper_tts,
            commands::get_dynamic_atmosphere,
            commands::get_active_window_info,
            commands::get_audio_proxy_port,
            commands::get_local_folders,
            commands::pick_and_add_local_folder,
            commands::remove_local_folder,
            commands::scan_local_tracks,
            commands::update_smtc_state,
            commands::set_track_cache_enabled,
            commands::report_wave_feedback,
            commands::open_external_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
