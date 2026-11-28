use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::watch;

use crate::auth::SessionStore;
use crate::config::Config;
use crate::limits::IpGuard;
use crate::rooms::Registry;
use crate::telemetry::Counters;

pub type Shared = Arc<AppState>;

#[derive(Debug)]
pub struct AppState {
    
    pub config: Config,
    
    pub counters: Arc<Counters>,
    
    pub rooms: Registry,
    
    pub sessions: SessionStore,
    
    pub ip_guard: Arc<IpGuard>,
    started: Instant,
    stopping: watch::Sender<bool>,
}

impl AppState {
    
    pub fn new(config: Config) -> Self {
        let counters = Arc::new(Counters::default());
        let ip_guard = IpGuard::new(&config.limits);
        let rooms = Registry::new(config.limits.clone(), Arc::clone(&counters));
        let (stopping, _) = watch::channel(false);

        Self {
            config,
            counters,
            rooms,
            sessions: SessionStore::new(),
            ip_guard,
            started: Instant::now(),
            stopping,
        }
    }

    
    pub fn shared(config: Config) -> Shared {
        Arc::new(Self::new(config))
    }

    
    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    
    pub fn begin_shutdown(&self) {
        self.stopping.send_replace(true);
        self.rooms.close_all();
    }

    
    pub fn shutdown_watch(&self) -> watch::Receiver<bool> {
        self.stopping.subscribe()
    }

    
    pub fn stopping(&self) -> bool {
        *self.stopping.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_is_empty() {
        let state = AppState::new(Config::default());

        assert!(state.rooms.is_empty());
        assert!(state.sessions.is_empty());
        assert!(!state.stopping());
    }

    #[test]
    fn shutdown_is_visible_to_subscribers() {
        let state = AppState::new(Config::default());
        let watcher = state.shutdown_watch();

        state.begin_shutdown();

        assert!(state.stopping());
        assert!(*watcher.borrow());
    }
}
