use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::config::Limits;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpKey([u8; 16]);

#[derive(Debug, Default, Clone, Copy)]
struct Slots {
    connections: usize,
    rooms: usize,
}

impl Slots {
    fn is_empty(self) -> bool {
        self.connections == 0 && self.rooms == 0
    }
}

#[derive(Debug)]
pub struct IpGuard {
    slots: Mutex<HashMap<IpKey, Slots>>,
    salt: [u8; 32],
    max_connections: usize,
    max_rooms: usize,
}

impl IpGuard {
    
    pub fn new(limits: &Limits) -> Arc<Self> {
        let mut salt = [0u8; 32];
        OsRng.fill_bytes(&mut salt);
        Self::with_salt(limits, salt)
    }

    
    pub fn with_salt(limits: &Limits, salt: [u8; 32]) -> Arc<Self> {
        Arc::new(Self {
            slots: Mutex::new(HashMap::new()),
            salt,
            max_connections: limits.max_connections_per_ip,
            max_rooms: limits.max_rooms_per_ip,
        })
    }

    
    pub fn key(&self, address: IpAddr) -> IpKey {
        let mut hasher = Sha256::new();
        hasher.update(self.salt);
        match address {
            IpAddr::V4(value) => hasher.update(value.octets()),
            IpAddr::V6(value) => hasher.update(value.octets()),
        }

        let digest = hasher.finalize();
        let mut key = [0u8; 16];
        key.copy_from_slice(&digest[..16]);
        IpKey(key)
    }

    
    pub fn acquire_connection(self: &Arc<Self>, key: IpKey) -> Option<ConnectionLease> {
        let mut slots = self.slots();
        let entry = slots.entry(key).or_default();

        if entry.connections >= self.max_connections {
            return None;
        }

        entry.connections += 1;
        drop(slots);

        Some(ConnectionLease {
            guard: Arc::clone(self),
            key,
        })
    }

    
    pub fn acquire_room(self: &Arc<Self>, key: IpKey) -> Option<RoomLease> {
        let mut slots = self.slots();
        let entry = slots.entry(key).or_default();

        if entry.rooms >= self.max_rooms {
            return None;
        }

        entry.rooms += 1;
        drop(slots);

        Some(RoomLease {
            guard: Arc::clone(self),
            key,
        })
    }

    
    pub fn tracked(&self) -> usize {
        self.slots().len()
    }

    fn slots(&self) -> MutexGuard<'_, HashMap<IpKey, Slots>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self, key: IpKey, connection: bool) {
        let mut slots = self.slots();
        let Some(entry) = slots.get_mut(&key) else {
            return;
        };

        if connection {
            entry.connections = entry.connections.saturating_sub(1);
        } else {
            entry.rooms = entry.rooms.saturating_sub(1);
        }

        if entry.is_empty() {
            slots.remove(&key);
        }
    }
}

#[derive(Debug)]
pub struct ConnectionLease {
    guard: Arc<IpGuard>,
    key: IpKey,
}

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        self.guard.release(self.key, true);
    }
}

#[derive(Debug)]
pub struct RoomLease {
    guard: Arc<IpGuard>,
    key: IpKey,
}

impl Drop for RoomLease {
    fn drop(&mut self) {
        self.guard.release(self.key, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard(connections: usize, rooms: usize) -> Arc<IpGuard> {
        let limits = Limits {
            max_connections_per_ip: connections,
            max_rooms_per_ip: rooms,
            ..Limits::default()
        };
        IpGuard::with_salt(&limits, [7u8; 32])
    }

    fn address() -> IpAddr {
        "203.0.113.7".parse().expect("адрес разбирается")
    }

    #[test]
    fn key_hides_the_address() {
        let guard = guard(2, 1);
        let key = guard.key(address());
        assert_ne!(key.0, [0u8; 16]);

        let other = IpGuard::with_salt(&Limits::default(), [8u8; 32]);
        assert_ne!(key, other.key(address()));
    }

    #[test]
    fn connection_limit_holds() {
        let guard = guard(2, 1);
        let key = guard.key(address());

        let first = guard.acquire_connection(key);
        let second = guard.acquire_connection(key);
        assert!(first.is_some() && second.is_some());
        assert!(guard.acquire_connection(key).is_none());

        drop(first);
        assert!(guard.acquire_connection(key).is_some());
    }

    #[test]
    fn room_limit_is_counted_apart() {
        let guard = guard(2, 1);
        let key = guard.key(address());

        let room = guard.acquire_room(key);
        assert!(room.is_some());
        assert!(guard.acquire_room(key).is_none());
        assert!(guard.acquire_connection(key).is_some());
    }

    #[test]
    fn empty_entries_are_removed() {
        let guard = guard(2, 1);
        let key = guard.key(address());

        {
            let _lease = guard.acquire_connection(key);
            assert_eq!(guard.tracked(), 1);
        }

        assert_eq!(guard.tracked(), 0);
    }
}
