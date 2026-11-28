use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use together_crypto::encoding::to_base64;

#[derive(Debug, Clone, Copy)]
struct Entry {
    public: [u8; 32],
    deadline: Instant,
}

#[derive(Debug, Default)]
pub struct SessionStore {
    entries: Mutex<HashMap<[u8; 32], Entry>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    
    pub fn issue(&self, public: [u8; 32], ttl: Duration) -> String {
        self.issue_at(public, ttl, Instant::now())
    }

    
    pub fn issue_at(&self, public: [u8; 32], ttl: Duration, now: Instant) -> String {
        let mut raw = [0u8; 32];
        OsRng.fill_bytes(&mut raw);
        let token = to_base64(&raw);

        self.entries().insert(
            digest(&token),
            Entry {
                public,
                deadline: now + ttl,
            },
        );

        token
    }

    
    
    
    pub fn resume(&self, token: &str, now: Instant) -> Option<[u8; 32]> {
        let entry = self.entries().remove(&digest(token))?;

        if entry.deadline <= now {
            return None;
        }

        Some(entry.public)
    }

    
    pub fn forget(&self, token: &str) {
        self.entries().remove(&digest(token));
    }

    
    pub fn sweep(&self, now: Instant) {
        self.entries().retain(|_, entry| entry.deadline > now);
    }

    
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<[u8; 32], Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn digest(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC: [u8; 32] = [5u8; 32];

    #[test]
    fn issued_token_works_once() {
        let store = SessionStore::new();
        let now = Instant::now();
        let token = store.issue_at(PUBLIC, Duration::from_secs(60), now);

        assert_eq!(store.resume(&token, now), Some(PUBLIC));
        assert_eq!(store.resume(&token, now), None);
        assert!(store.is_empty());
    }

    #[test]
    fn expired_token_is_refused() {
        let store = SessionStore::new();
        let now = Instant::now();
        let token = store.issue_at(PUBLIC, Duration::from_secs(10), now);

        assert_eq!(store.resume(&token, now + Duration::from_secs(11)), None);
    }

    #[test]
    fn unknown_token_is_refused() {
        let store = SessionStore::new();
        assert_eq!(store.resume("чужой", Instant::now()), None);
    }

    #[test]
    fn sweep_removes_only_old_entries() {
        let store = SessionStore::new();
        let now = Instant::now();
        store.issue_at(PUBLIC, Duration::from_secs(5), now);
        let fresh = store.issue_at(PUBLIC, Duration::from_secs(600), now);

        store.sweep(now + Duration::from_secs(10));
        assert_eq!(store.len(), 1);
        assert_eq!(store.resume(&fresh, now + Duration::from_secs(10)), Some(PUBLIC));
    }

    #[test]
    fn tokens_do_not_repeat() {
        let store = SessionStore::new();
        let first = store.issue(PUBLIC, Duration::from_secs(60));
        let second = store.issue(PUBLIC, Duration::from_secs(60));
        assert_ne!(first, second);
    }
}
