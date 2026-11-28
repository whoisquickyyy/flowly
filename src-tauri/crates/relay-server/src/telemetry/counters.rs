use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Counters {
    connections_total: AtomicU64,
    connections_open: AtomicU64,
    rooms_total: AtomicU64,
    rooms_open: AtomicU64,
    messages_relayed: AtomicU64,
    handshakes_failed: AtomicU64,
    rate_limited: AtomicU64,
    errors_total: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub connections_total: u64,
    pub connections_open: u64,
    pub rooms_total: u64,
    pub rooms_open: u64,
    pub messages_relayed: u64,
    pub handshakes_failed: u64,
    pub rate_limited: u64,
    pub errors_total: u64,
}

impl Counters {
    pub fn connection_opened(&self) {
        self.connections_total.fetch_add(1, Ordering::Relaxed);
        self.connections_open.fetch_add(1, Ordering::Relaxed);
    }

    pub fn connection_closed(&self) {
        decrement(&self.connections_open);
    }

    pub fn room_opened(&self) {
        self.rooms_total.fetch_add(1, Ordering::Relaxed);
        self.rooms_open.fetch_add(1, Ordering::Relaxed);
    }

    pub fn room_closed(&self) {
        decrement(&self.rooms_open);
    }

    pub fn message_relayed(&self, receivers: u64) {
        self.messages_relayed.fetch_add(receivers, Ordering::Relaxed);
    }

    pub fn handshake_failed(&self) {
        self.handshakes_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn rate_limited(&self) {
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    pub fn error_sent(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            connections_total: self.connections_total.load(Ordering::Relaxed),
            connections_open: self.connections_open.load(Ordering::Relaxed),
            rooms_total: self.rooms_total.load(Ordering::Relaxed),
            rooms_open: self.rooms_open.load(Ordering::Relaxed),
            messages_relayed: self.messages_relayed.load(Ordering::Relaxed),
            handshakes_failed: self.handshakes_failed.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
            errors_total: self.errors_total.load(Ordering::Relaxed),
        }
    }
}

fn decrement(value: &AtomicU64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_sub(1))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_close_are_balanced() {
        let counters = Counters::default();
        counters.connection_opened();
        counters.connection_opened();
        counters.connection_closed();

        let snapshot = counters.snapshot();
        assert_eq!(snapshot.connections_total, 2);
        assert_eq!(snapshot.connections_open, 1);
    }

    #[test]
    fn gauge_does_not_go_below_zero() {
        let counters = Counters::default();
        counters.room_closed();
        assert_eq!(counters.snapshot().rooms_open, 0);
    }

    #[test]
    fn relay_counts_receivers() {
        let counters = Counters::default();
        counters.message_relayed(3);
        counters.message_relayed(2);
        assert_eq!(counters.snapshot().messages_relayed, 5);
    }
}
