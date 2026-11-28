use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use serde_json::Value;

use crate::config::Limits;
use crate::limits::RoomLease;
use crate::rooms::room::{Room, Seat};
use crate::telemetry::Counters;
use crate::wire::{ErrorCode, ServerMessage};
use crate::ws::Outbox;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    
    pub room: String,
    
    pub seat: Seat,
    
    pub host: u64,
    
    pub created: bool,
}

#[derive(Debug)]
pub struct Registry {
    rooms: Mutex<HashMap<String, Room>>,
    limits: Limits,
    counters: Arc<Counters>,
}

impl Registry {
    pub fn new(limits: Limits, counters: Arc<Counters>) -> Self {
        Self {
            rooms: Mutex::new(HashMap::new()),
            limits,
            counters,
        }
    }

    
    
    
    
    
    
    
    pub fn create(
        &self,
        id: String,
        invite: Option<String>,
        lease: RoomLease,
        public: [u8; 32],
        nick: String,
        outbox: Outbox,
        now: Instant,
    ) -> Result<Joined, ErrorCode> {
        let mut rooms = self.rooms();
        let mut created = false;

        let room = match rooms.entry(id.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                created = true;
                self.counters.room_opened();
                entry.insert(Room::new(
                    id.clone(),
                    invite,
                    lease,
                    self.limits.max_peers_per_room,
                    now,
                ))
            }
        };

        match room.join(public, nick, outbox, now) {
            Ok(seat) => {
                let host = room.host();

                Ok(Joined {
                    room: id,
                    seat,
                    host,
                    created,
                })
            }
            Err(error) => {
                
                if created {
                    rooms.remove(&id);
                    self.counters.room_closed();
                }

                Err(error)
            }
        }
    }

    
    pub fn join(
        &self,
        id: &str,
        public: [u8; 32],
        nick: String,
        outbox: Outbox,
        now: Instant,
    ) -> Result<Joined, ErrorCode> {
        let mut rooms = self.rooms();
        let room = rooms.get_mut(id).ok_or(ErrorCode::NoRoom)?;

        let seat = room.join(public, nick, outbox, now)?;
        let host = room.host();

        Ok(Joined {
            room: id.to_string(),
            seat,
            host,
            created: false,
        })
    }

    
    pub fn greet(&self, id: &str, seat: Seat) {
        let rooms = self.rooms();
        let Some(room) = rooms.get(id) else {
            return;
        };

        room.send_to(
            seat.id,
            &ServerMessage::Welcome {
                id: seat.id,
                room: id.to_string(),
                host: room.host(),
            },
        );

        room.announce_peers();
    }

    
    pub fn relay(&self, id: &str, seat: Seat, payload: Value) -> Result<u64, ErrorCode> {
        let rooms = self.rooms();
        let room = rooms.get(id).ok_or(ErrorCode::NoRoom)?;

        let delivered = room.relay(seat.id, payload)?;
        drop(rooms);

        self.counters.message_relayed(delivered);
        Ok(delivered)
    }

    
    pub fn handoff(&self, id: &str, seat: Seat, to: u64) -> Result<u64, ErrorCode> {
        let mut rooms = self.rooms();
        let room = rooms.get_mut(id).ok_or(ErrorCode::NoRoom)?;

        room.handoff(seat.id, to)?;
        room.announce_host();
        room.announce_peers();

        Ok(room.host())
    }

    
    pub fn leave(&self, id: &str, seat: Seat, now: Instant) {
        let mut rooms = self.rooms();
        let Some(room) = rooms.get_mut(id) else {
            return;
        };

        let outcome = room.leave(seat, now);
        if outcome.host_changed {
            room.announce_host();
        }

        if outcome.roster_changed {
            room.announce_peers();
        }
    }

    
    pub fn detach(&self, id: &str, seat: Seat, now: Instant) {
        let mut rooms = self.rooms();
        let Some(room) = rooms.get_mut(id) else {
            return;
        };

        if room.detach(seat, now) {
            room.announce_peers();
        }
    }

    
    pub fn sweep(&self, now: Instant) {
        let grace = self.limits.host_grace();
        let ttl = self.limits.empty_room_ttl();

        let mut closed = 0;
        let mut rooms = self.rooms();

        rooms.retain(|_, room| {
            let outcome = room.sweep(grace, now);
            if outcome.host_changed {
                room.announce_host();
            }

            if outcome.roster_changed {
                room.announce_peers();
            }

            let expired = room
                .empty_since()
                .is_some_and(|since| now.saturating_duration_since(since) >= ttl);

            if expired {
                room.release();
                closed += 1;
            }

            !expired
        });

        drop(rooms);

        for _ in 0..closed {
            self.counters.room_closed();
        }
    }

    
    pub fn close_all(&self) {
        let mut rooms = self.rooms();
        let closed = rooms.len();

        for room in rooms.values_mut() {
            room.release();
        }

        rooms.clear();
        drop(rooms);

        for _ in 0..closed {
            self.counters.room_closed();
        }
    }

    
    pub fn len(&self) -> usize {
        self.rooms().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    
    pub fn peers_in(&self, id: &str) -> usize {
        self.rooms().get(id).map_or(0, Room::len)
    }

    
    pub fn host_of(&self, id: &str) -> Option<u64> {
        self.rooms().get(id).map(Room::host)
    }

    fn rooms(&self) -> MutexGuard<'_, HashMap<String, Room>> {
        self.rooms.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{IpGuard, RoomLease};
    use crate::ws::Frame;
    use serde_json::json;
    use std::net::IpAddr;
    use tokio::sync::mpsc::UnboundedReceiver;

    const ROOM: &str = "abcdefghij234567";

    fn registry(max_peers: usize) -> (Registry, Arc<IpGuard>) {
        let limits = Limits {
            max_peers_per_room: max_peers,
            ..Limits::default()
        };

        let guard = IpGuard::with_salt(&limits, [3u8; 32]);
        let registry = Registry::new(limits, Arc::new(Counters::default()));
        (registry, guard)
    }

    fn lease(guard: &Arc<IpGuard>) -> RoomLease {
        let address: IpAddr = "198.51.100.9".parse().expect("адрес разбирается");
        guard
            .acquire_room(guard.key(address))
            .expect("слот есть")
    }

    fn drain(receiver: &mut UnboundedReceiver<Frame>) -> Vec<String> {
        let mut frames = Vec::new();
        while let Ok(Frame::Text(text)) = receiver.try_recv() {
            frames.push(text);
        }
        frames
    }

    #[test]
    fn creator_becomes_host_and_gets_welcome() {
        let (registry, guard) = registry(16);
        let (outbox, mut receiver) = Outbox::new();

        let joined = registry
            .create(
                ROOM.to_string(),
                Some("шесть слов".to_string()),
                lease(&guard),
                [1u8; 32],
                "nick1".to_string(),
                outbox,
                Instant::now(),
            )
            .expect("комната создана");

        assert!(joined.created);
        assert_eq!(joined.host, joined.seat.id);

        registry.greet(&joined.room, joined.seat);

        let frames = drain(&mut receiver);
        assert!(frames[0].contains(r#""t":"welcome""#));
        assert!(frames[1].contains(r#""t":"peers""#));
    }

    #[test]
    fn second_create_with_same_ticket_joins() {
        let (registry, guard) = registry(16);
        let (first, _one) = Outbox::new();
        let (second, _two) = Outbox::new();
        let now = Instant::now();

        registry
            .create(
                ROOM.to_string(),
                None,
                lease(&guard),
                [1u8; 32],
                "nick1".to_string(),
                first,
                now,
            )
            .expect("комната создана");

        let joined = registry
            .create(
                ROOM.to_string(),
                None,
                lease(&guard),
                [2u8; 32],
                "nick2".to_string(),
                second,
                now,
            )
            .expect("вход в ту же комнату");

        assert!(!joined.created);
        assert_eq!(joined.host, 1);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.peers_in(ROOM), 2);
    }

    #[test]
    fn join_without_room_is_refused() {
        let (registry, _guard) = registry(16);
        let (outbox, _receiver) = Outbox::new();

        let result = registry.join(
            ROOM,
            [1u8; 32],
            "nick1".to_string(),
            outbox,
            Instant::now(),
        );

        assert_eq!(result, Err(ErrorCode::NoRoom));
    }

    #[test]
    fn relay_reaches_only_the_others() {
        let (registry, guard) = registry(16);
        let (first, mut one) = Outbox::new();
        let (second, mut two) = Outbox::new();
        let now = Instant::now();

        let host = registry
            .create(
                ROOM.to_string(),
                None,
                lease(&guard),
                [1u8; 32],
                "nick1".to_string(),
                first,
                now,
            )
            .expect("комната создана");

        registry
            .join(ROOM, [2u8; 32], "nick2".to_string(), second, now)
            .expect("вход разрешён");

        drain(&mut one);
        drain(&mut two);

        let delivered = registry
            .relay(ROOM, host.seat, json!({"enc": "AAAA"}))
            .expect("отправитель в комнате");

        assert_eq!(delivered, 1);
        assert!(drain(&mut one).is_empty());
        assert_eq!(drain(&mut two).len(), 1);
    }

    #[test]
    fn handoff_moves_the_lead() {
        let (registry, guard) = registry(16);
        let (first, _one) = Outbox::new();
        let (second, _two) = Outbox::new();
        let now = Instant::now();

        let host = registry
            .create(
                ROOM.to_string(),
                None,
                lease(&guard),
                [1u8; 32],
                "nick1".to_string(),
                first,
                now,
            )
            .expect("комната создана");

        let guest = registry
            .join(ROOM, [2u8; 32], "nick2".to_string(), second, now)
            .expect("вход разрешён");

        let lead = registry
            .handoff(ROOM, host.seat, guest.seat.id)
            .expect("хост отдаёт комнату");

        assert_eq!(lead, guest.seat.id);
        assert_eq!(registry.host_of(ROOM), Some(guest.seat.id));
        assert_eq!(
            registry.handoff(ROOM, host.seat, guest.seat.id),
            Err(ErrorCode::NotHost)
        );
    }

    #[test]
    fn empty_room_dies_after_ttl() {
        let (registry, guard) = registry(16);
        let (outbox, _receiver) = Outbox::new();
        let now = Instant::now();

        let joined = registry
            .create(
                ROOM.to_string(),
                None,
                lease(&guard),
                [1u8; 32],
                "nick1".to_string(),
                outbox,
                now,
            )
            .expect("комната создана");

        registry.leave(ROOM, joined.seat, now);
        assert_eq!(registry.len(), 1);

        registry.sweep(now + Limits::default().empty_room_ttl());
        assert!(registry.is_empty());
    }
}
