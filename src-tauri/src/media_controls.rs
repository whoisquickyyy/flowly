use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};
use tauri::{AppHandle, Emitter, Manager};

pub enum SmtcCommand {
    Metadata {
        title: String,
        artist: String,
        album: String,
        cover_url: Option<String>,
        duration_sec: u32,
    },
    Playback {
        playing: bool,
        position_sec: u32,
    },
}

pub struct SmtcHandle {
    tx: Mutex<Option<Sender<SmtcCommand>>>,
}

impl SmtcHandle {
    pub fn spawn(app: &AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<SmtcCommand>();
        let handle = app.clone();

        std::thread::Builder::new()
            .name("flowly-smtc".into())
            .spawn(move || {
                #[cfg(not(target_os = "windows"))]
                let hwnd = None;

                #[cfg(target_os = "windows")]
                let hwnd = handle.get_webview_window("main").and_then(|win| {
                    use raw_window_handle::HasWindowHandle;
                    win.window_handle().ok().and_then(|wh| match wh.as_raw() {
                        raw_window_handle::RawWindowHandle::Win32(h) => {
                            Some(h.hwnd.get() as *mut std::ffi::c_void)
                        }
                        _ => None,
                    })
                });

                let config = PlatformConfig {
                    display_name: "Flowly",
                    dbus_name: "flowly",
                    hwnd,
                };

                let Ok(mut controls) = MediaControls::new(config) else {
                    return;
                };

                let emit_handle = handle.clone();
                let _ = controls.attach(move |event: MediaControlEvent| {
                    match event {
                        MediaControlEvent::Play => {
                            let _ = emit_handle.emit("media:play", ());
                        }
                        MediaControlEvent::Pause => {
                            let _ = emit_handle.emit("media:pause", ());
                        }
                        MediaControlEvent::Toggle => {
                            let _ = emit_handle.emit("media:toggle", ());
                        }
                        MediaControlEvent::Next => {
                            let _ = emit_handle.emit("media:next", ());
                        }
                        MediaControlEvent::Previous => {
                            let _ = emit_handle.emit("media:prev", ());
                        }
                        MediaControlEvent::Stop => {
                            let _ = emit_handle.emit("media:pause", ());
                        }
                        _ => {}
                    }
                });

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        SmtcCommand::Metadata {
                            title,
                            artist,
                            album,
                            cover_url,
                            duration_sec,
                        } => {
                            let dur = if duration_sec > 0 {
                                Some(std::time::Duration::from_secs(duration_sec as u64))
                            } else {
                                None
                            };
                            let _ = controls.set_metadata(MediaMetadata {
                                title: Some(&title),
                                artist: Some(&artist),
                                album: Some(&album),
                                cover_url: cover_url.as_deref(),
                                duration: dur,
                            });
                        }
                        SmtcCommand::Playback {
                            playing,
                            position_sec,
                        } => {
                            let pos = Some(MediaPosition(std::time::Duration::from_secs(
                                position_sec as u64,
                            )));
                            let pb = if playing {
                                MediaPlayback::Playing { progress: pos }
                            } else {
                                MediaPlayback::Paused { progress: pos }
                            };
                            let _ = controls.set_playback(pb);
                        }
                    }
                }
            })
            .ok();

        Self {
            tx: Mutex::new(Some(tx)),
        }
    }

    pub fn send(&self, cmd: SmtcCommand) {
        if let Ok(guard) = self.tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(cmd);
            }
        }
    }
}
