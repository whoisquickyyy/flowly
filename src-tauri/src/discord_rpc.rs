use std::sync::mpsc::{self, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

use crate::models::{PlayerState, Track};

pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1575481560205885641";

pub enum RpcMessage {
    Configure {
        enabled: bool,
        client_id: String,
    },
    Update {
        track: Option<Track>,
        is_playing: bool,
        position_sec: u32,
    },
}

pub struct DiscordRpcHandle {
    tx: Sender<RpcMessage>,
}

impl DiscordRpcHandle {
    pub fn spawn(initial_enabled: bool, initial_client_id: String) -> Self {
        let (tx, rx) = mpsc::channel::<RpcMessage>();

        std::thread::spawn(move || {
            let mut enabled = initial_enabled;
            let mut client_id = if initial_client_id.trim().is_empty() {
                DEFAULT_DISCORD_CLIENT_ID.to_string()
            } else {
                initial_client_id
            };
            let mut client: Option<DiscordIpcClient> = None;

            let ensure_connected =
                |cli: &mut Option<DiscordIpcClient>, cid: &str| -> bool {
                    let effective = if cid.trim().is_empty() {
                        DEFAULT_DISCORD_CLIENT_ID
                    } else {
                        cid.trim()
                    };
                    if cli.is_some() {
                        return true;
                    }
                    let mut c = DiscordIpcClient::new(effective);
                    if c.connect().is_ok() {
                        *cli = Some(c);
                        true
                    } else {
                        false
                    }
                };

            let clear_activity = |cli: &mut Option<DiscordIpcClient>| {
                if let Some(ref mut c) = cli {
                    let _ = c.clear_activity();
                }
            };

            while let Ok(msg) = rx.recv() {
                match msg {
                    RpcMessage::Configure {
                        enabled: new_en,
                        client_id: new_cid,
                    } => {
                        let effective_new = if new_cid.trim().is_empty() {
                            DEFAULT_DISCORD_CLIENT_ID.to_string()
                        } else {
                            new_cid.trim().to_string()
                        };
                        let cid_changed = client_id != effective_new;
                        enabled = new_en;
                        client_id = effective_new;
                        if !enabled || cid_changed {
                            clear_activity(&mut client);
                            if let Some(mut c) = client.take() {
                                let _ = c.close();
                            }
                        }
                    }
                    RpcMessage::Update {
                        track,
                        is_playing,
                        position_sec,
                    } => {
                        if !enabled {
                            clear_activity(&mut client);
                            continue;
                        }
                        let Some(tr) = track else {
                            clear_activity(&mut client);
                            continue;
                        };
                        if !is_playing {
                            clear_activity(&mut client);
                            continue;
                        }
                        if !ensure_connected(&mut client, &client_id) {
                            continue;
                        }

                        let now_sec = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        let start_ts = now_sec - (position_sec as i64);
                        let end_ts = if tr.duration > 0 {
                            Some(start_ts + tr.duration as i64)
                        } else {
                            None
                        };

                        let mut ts = activity::Timestamps::new().start(start_ts);
                        if let Some(e) = end_ts {
                            ts = ts.end(e);
                        }

                        let details_str = if tr.title.len() < 2 {
                            format!("{} ", tr.title)
                        } else {
                            tr.title.clone()
                        };
                        let state_str = format!("{} · {}", tr.artist, tr.source.display_name());
                        let mut assets = activity::Assets::new()
                            .large_text(if tr.album.is_empty() {
                                tr.title.as_str()
                            } else {
                                tr.album.as_str()
                            });
                        if tr.artwork.starts_with("http") {
                            assets = assets.large_image(tr.artwork.as_str());
                        }

                        let act = activity::Activity::new()
                            .activity_type(activity::ActivityType::Listening)
                            .details(details_str.as_str())
                            .state(state_str.as_str())
                            .assets(assets)
                            .timestamps(ts);

                        if let Some(ref mut c) = client {
                            if c.set_activity(act).is_err() {
                                // Переподключаемся, если Discord перезапустился
                                let _ = c.close();
                                client = None;
                                if ensure_connected(&mut client, &client_id) {
                                    if let Some(ref mut c2) = client {
                                        let mut ts2 = activity::Timestamps::new().start(start_ts);
                                        if let Some(e) = end_ts {
                                            ts2 = ts2.end(e);
                                        }
                                        let act2 = activity::Activity::new()
                                            .activity_type(activity::ActivityType::Listening)
                                            .details(details_str.as_str())
                                            .state(state_str.as_str())
                                            .timestamps(ts2);
                                        let _ = c2.set_activity(act2);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    pub fn configure(&self, enabled: bool, client_id: String) {
        let _ = self.tx.send(RpcMessage::Configure { enabled, client_id });
    }

    pub fn update_from_player(&self, player: &PlayerState) {
        let _ = self.tx.send(RpcMessage::Update {
            track: player.current_track.clone(),
            is_playing: player.is_playing,
            position_sec: player.position_sec,
        });
    }
}
