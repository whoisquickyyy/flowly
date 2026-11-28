use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use crate::app::Shared;

pub const TICK: Duration = Duration::from_secs(1);

pub fn spawn(state: Shared) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            let now = Instant::now();
            state.rooms.sweep(now);
            state.sessions.sweep(now);
        }
    })
}
