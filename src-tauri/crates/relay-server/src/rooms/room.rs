use std::time::{Duration, Instant};

use serde_json::Value;

use crate::limits::RoomLease;
use crate::rooms::peer::Peer;
use crate::wire::{ErrorCode, PeerView, ServerMessage};
use crate::ws::Outbox;

pub const NO_HOST: u64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seat {
    pub id: u64,
    pub epoch: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepOutcome {
    
    pub roster_changed: bool,
    
    pub host_changed: bool,
}

#[derive(Debug)]
pub struct Room {
    id: String,
    invite: Option<String>,
    host: u64,
    next_id: u64,
    peers: Vec<Peer>,
    empty_since: Option<Instant>,
    max_peers: usize,
    lease: Option<RoomLease>,
}

impl Room {
    
    pub fn new(
        id: String,
        invite: Option<String>,
        lease: RoomLease,
        max_peers: usize,
        now: Instant,
    ) -> Self {
        Self {
            id,
            invite,
            host: NO_HOST,
            next_id: 1,
            peers: Vec::new(),
            empty_since: Some(now),
            max_peers,
            lease: Some(lease),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    
    pub fn invite(&self) -> Option<&str> {
        self.invite.as_deref()
    }

    pub fn host(&self) -> u64 {
        self.host
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    
    pub fn empty_since(&self) -> Option<Instant> {
        self.empty_since
    }

    
    pub fn views(&self) -> Vec<PeerView> {
        self.peers.iter().map(Peer::view).collect()
    }

    
    pub fn join(
        &mut self,
        public: [u8; 32],
        nick: String,
        outbox: Outbox,
        now: Instant,
    ) -> Result<Seat, ErrorCode> {
        self.empty_since = None;

        if let Some(peer) = self.peers.iter_mut().find(|peer| peer.public() == public) {
            let epoch = peer.attach(nick, outbox);
            return Ok(Seat {
                id: peer.id(),
                epoch,
            });
        }

        if self.peers.len() >= self.max_peers {
            return Err(ErrorCode::Full);
        }

        let id = self.next_id;
        self.next_id += 1;

        let peer = Peer::new(id, public, nick, outbox, now);
        let epoch = peer.epoch();
        self.peers.push(peer);

        if self.host == NO_HOST {
            self.host = id;
        }

        Ok(Seat { id, epoch })
    }

    
    pub fn leave(&mut self, seat: Seat, now: Instant) -> SweepOutcome {
        let before = self.peers.len();
        self.peers
            .retain(|peer| !(peer.id() == seat.id && peer.matches(seat.epoch)));

        if self.peers.len() == before {
            return SweepOutcome::default();
        }

        if self.peers.is_empty() {
            self.empty_since = Some(now);
        }

        let host_changed = self.fix_host();
        SweepOutcome {
            roster_changed: true,
            host_changed,
        }
    }

    
    pub fn detach(&mut self, seat: Seat, now: Instant) -> bool {
        let Some(peer) = self.peers.iter_mut().find(|peer| peer.id() == seat.id) else {
            return false;
        };

        peer.detach(seat.epoch, now)
    }

    
    pub fn relay(&self, from: u64, payload: Value) -> Result<u64, ErrorCode> {
        let Some(sender) = self.peers.iter().find(|peer| peer.id() == from) else {
            return Err(ErrorCode::NoPeer);
        };

        let message = ServerMessage::Msg {
            from,
            nick: sender.nick().to_string(),
            payload,
        };

        let mut delivered = 0;
        for peer in self.peers.iter().filter(|peer| peer.id() != from) {
            if peer.send(&message) {
                delivered += 1;
            }
        }

        Ok(delivered)
    }

    
    pub fn handoff(&mut self, from: u64, to: u64) -> Result<(), ErrorCode> {
        if self.host != from {
            return Err(ErrorCode::NotHost);
        }

        let target_online = self
            .peers
            .iter()
            .any(|peer| peer.id() == to && peer.online());

        if !target_online {
            return Err(ErrorCode::NoPeer);
        }

        self.host = to;
        Ok(())
    }

    
    pub fn sweep(&mut self, grace: Duration, now: Instant) -> SweepOutcome {
        let before = self.peers.len();

        self.peers.retain(|peer| match peer.offline_since() {
            Some(since) => now.saturating_duration_since(since) < grace,
            None => true,
        });

        let roster_changed = self.peers.len() != before;

        if self.peers.is_empty() && self.empty_since.is_none() {
            self.empty_since = Some(now);
        }

        let host_changed = self.host_needs_change(grace, now) && self.fix_host();

        SweepOutcome {
            roster_changed: roster_changed || host_changed,
            host_changed,
        }
    }

    
    pub fn send_to(&self, id: u64, message: &ServerMessage) -> bool {
        self.peers
            .iter()
            .find(|peer| peer.id() == id)
            .is_some_and(|peer| peer.send(message))
    }

    
    pub fn announce_peers(&self) {
        let message = ServerMessage::Peers {
            peers: self.views(),
            host: self.host,
        };

        self.broadcast(&message);
    }

    
    pub fn announce_host(&self) {
        self.broadcast(&ServerMessage::Host { id: self.host });
    }

    
    pub fn broadcast(&self, message: &ServerMessage) {
        for peer in &self.peers {
            peer.send(message);
        }
    }

    
    pub fn release(&mut self) {
        for peer in &self.peers {
            peer.close();
        }

        self.peers.clear();
        self.lease = None;
    }

    
    fn host_needs_change(&self, grace: Duration, now: Instant) -> bool {
        match self.peers.iter().find(|peer| peer.id() == self.host) {
            Some(host) => host
                .offline_since()
                .is_some_and(|since| now.saturating_duration_since(since) >= grace),
            None => true,
        }
    }

    
    fn fix_host(&mut self) -> bool {
        let alive = self
            .peers
            .iter()
            .any(|peer| peer.id() == self.host && peer.online());

        if alive {
            return false;
        }

        let next = self
            .peers
            .iter()
            .filter(|peer| peer.online())
            .min_by_key(|peer| peer.joined_at())
            .map(Peer::id)
            .unwrap_or(NO_HOST);

        if next == self.host {
            return false;
        }

        self.host = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Limits;
    use crate::limits::IpGuard;
    use serde_json::json;
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::ws::Frame;

    fn room(max_peers: usize) -> Room {
        let limits = Limits::default();
        let guard = IpGuard::with_salt(&limits, [1u8; 32]);
        let key = guard.key("198.51.100.1".parse().expect("адрес разбирается"));
        let lease = guard.acquire_room(key).expect("слот есть");

        Room::new(
            "abcdefghij234567".to_string(),
            None,
            lease,
            max_peers,
            Instant::now(),
        )
    }

    fn join(room: &mut Room, number: u8, now: Instant) -> (Seat, UnboundedReceiver<Frame>) {
        let (outbox, receiver) = Outbox::new();
        let seat = room
            .join([number; 32], format!("nick{number}"), outbox, now)
            .expect("место есть");
        (seat, receiver)
    }

    #[test]
    fn first_peer_becomes_host() {
        let mut room = room(16);
        let now = Instant::now();

        let (first, _one) = join(&mut room, 1, now);
        let (second, _two) = join(&mut room, 2, now);

        assert_eq!(room.host(), first.id);
        assert_eq!(first.id, 1);
        assert_eq!(second.id, 2);
    }

    #[test]
    fn full_room_refuses_newcomers() {
        let mut room = room(2);
        let now = Instant::now();

        join(&mut room, 1, now);
        join(&mut room, 2, now);

        let (outbox, _receiver) = Outbox::new();
        assert_eq!(
            room.join([3u8; 32], "nick3".to_string(), outbox, now),
            Err(ErrorCode::Full)
        );
    }

    #[test]
    fn relay_skips_the_sender() {
        let mut room = room(16);
        let now = Instant::now();

        let (first, mut one) = join(&mut room, 1, now);
        let (_second, mut two) = join(&mut room, 2, now);

        let delivered = room
            .relay(first.id, json!({"enc": "AAAA"}))
            .expect("отправитель в комнате");

        assert_eq!(delivered, 1);
        assert!(one.try_recv().is_err());
        assert!(two.try_recv().is_ok());
    }

    #[test]
    fn handoff_is_for_host_only() {
        let mut room = room(16);
        let now = Instant::now();

        let (first, _one) = join(&mut room, 1, now);
        let (second, _two) = join(&mut room, 2, now);

        assert_eq!(room.handoff(second.id, first.id), Err(ErrorCode::NotHost));
        assert_eq!(room.handoff(first.id, 99), Err(ErrorCode::NoPeer));
        assert!(room.handoff(first.id, second.id).is_ok());
        assert_eq!(room.host(), second.id);
    }

    #[test]
    fn host_is_replaced_after_grace() {
        let mut room = room(16);
        let now = Instant::now();
        let grace = Duration::from_secs(15);

        let (first, _one) = join(&mut room, 1, now);
        let (second, _two) = join(&mut room, 2, now + Duration::from_secs(1));

        assert!(room.detach(first, now + Duration::from_secs(2)));

        let early = room.sweep(grace, now + Duration::from_secs(10));
        assert!(!early.host_changed);
        assert_eq!(room.host(), first.id);

        let late = room.sweep(grace, now + Duration::from_secs(20));
        assert!(late.host_changed);
        assert_eq!(room.host(), second.id);
        assert_eq!(room.len(), 1);
    }

    #[test]
    fn returning_peer_keeps_number_and_host() {
        let mut room = room(16);
        let now = Instant::now();

        let (first, _one) = join(&mut room, 1, now);
        join(&mut room, 2, now);
        room.detach(first, now);

        let (outbox, _again) = Outbox::new();
        let seat = room
            .join([1u8; 32], "nick1".to_string(), outbox, now + Duration::from_secs(3))
            .expect("возвращение разрешено");

        assert_eq!(seat.id, first.id);
        assert_ne!(seat.epoch, first.epoch);
        assert_eq!(room.host(), first.id);
        assert_eq!(room.len(), 2);
    }

    #[test]
    fn leaving_last_peer_marks_room_empty() {
        let mut room = room(16);
        let now = Instant::now();

        let (first, _one) = join(&mut room, 1, now);
        let outcome = room.leave(first, now);

        assert!(outcome.roster_changed);
        assert!(room.is_empty());
        assert_eq!(room.empty_since(), Some(now));
        assert_eq!(room.host(), NO_HOST);
    }
}
