// Встроенный релей mashiro-together: поднимается внутри Flowly одной командой,
// чтобы хостить комнаты без отдельной программы.

use std::sync::Mutex;

use tauri::{AppHandle, State};
use tokio::sync::watch;

use crate::together::log;

pub struct RelayServerState {
    shutdown: Mutex<Option<watch::Sender<bool>>>,
}

impl Default for RelayServerState {
    fn default() -> Self {
        Self {
            shutdown: Mutex::new(None),
        }
    }
}

#[derive(Clone, serde::Serialize)]
pub struct RelayServerInfo {
    pub address: String,
    pub port: u16,
}

#[tauri::command]
pub async fn relay_server_start(
    app: AppHandle,
    state: State<'_, RelayServerState>,
    port: u16,
) -> Result<RelayServerInfo, String> {
    {
        let guard = state.shutdown.lock().unwrap();
        if guard.is_some() {
            return Err("Релей уже запущен".into());
        }
    }

    let mut config = relay::Config::default();
    config.server.address = "0.0.0.0".to_string();
    config.server.port = port;
    // Flowly-клиент всегда шлёт производный ticket — plain-код оставляем разрешённым
    // для гибкости (например, для проверки из браузера)
    config.privacy.allow_plain_invite = true;
    config.log.level = "warn".to_string();

    let server = relay::Server::bind(config)
        .await
        .map_err(|e| format!("Не удалось занять порт {port}: {e}"))?;
    let address = server.address().to_string();

    let (tx, rx) = watch::channel(false);
    {
        let mut guard = state.shutdown.lock().unwrap();
        *guard = Some(tx);
    }

    let shutdown = async move {
        loop {
            if *rx.borrow() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    };

    let log_app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = server.serve(shutdown).await {
            log::write(&log_app, "relay-server", &format!("релей остановлен с ошибкой: {e}"));
        }
    });

    log::write(&app, "relay-server", &format!("релей слушает {address}"));
    Ok(RelayServerInfo {
        address,
        port,
    })
}

#[tauri::command]
pub fn relay_server_stop(state: State<'_, RelayServerState>) -> Result<(), String> {
    let mut guard = state.shutdown.lock().unwrap();
    if let Some(tx) = guard.take() {
        let _ = tx.send(true);
    }
    Ok(())
}

#[tauri::command]
pub fn relay_server_running(state: State<'_, RelayServerState>) -> bool {
    state.shutdown.lock().unwrap().is_some()
}

// Локальный IP для подстановки в адрес релея (как local_ip из wire.rs)
#[tauri::command]
pub fn relay_server_local_ip() -> Option<String> {
    crate::together::wire::local_ip()
}
