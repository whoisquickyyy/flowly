use std::time::Instant;

use crate::wire::{PeerView, ServerMessage};
use crate::ws::Outbox;

#[derive(Debug)]
pub struct Peer {
    id: u64,
    public: [u8; 32],
    nick: String,
    joined_at: Instant,
    epoch: u64,
    outbox: Option<Outbox>,
    offline_since: Option<Instant>,
}

impl Peer {
    
    pub fn new(id: u64, public: [u8; 32], nick: String, outbox: Outbox, now: Instant) -> Self {
        Self {
            id,
            public,
            nick,
            joined_at: now,
            epoch: 1,
            outbox: Some(outbox),
            offline_since: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn public(&self) -> [u8; 32] {
        self.public
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn joined_at(&self) -> Instant {
        self.joined_at
    }

    
    pub fn online(&self) -> bool {
        self.outbox.as_ref().is_some_and(Outbox::is_open)
    }

    
    pub fn offline_since(&self) -> Option<Instant> {
        self.offline_since
    }

    
    pub fn view(&self) -> PeerView {
        PeerView {
            id: self.id,
            nick: self.nick.clone(),
            online: self.online(),
        }
    }

    
    pub fn nick(&self) -> &str {
        &self.nick
    }

    
    pub fn send(&self, message: &ServerMessage) -> bool {
        self.outbox.as_ref().is_some_and(|outbox| outbox.send(message))
    }

    
    pub fn close(&self) {
        if let Some(outbox) = &self.outbox {
            outbox.close();
        }
    }

    
    pub fn attach(&mut self, nick: String, outbox: Outbox) -> u64 {
        if !nick.is_empty() {
            self.nick = nick;
        }

        if let Some(previous) = self.outbox.replace(outbox) {
            
            previous.close();
        }

        self.offline_since = None;
        self.epoch += 1;
        self.epoch
    }

    
    pub fn detach(&mut self, epoch: u64, now: Instant) -> bool {
        if self.epoch != epoch {
            return false;
        }

        self.outbox = None;
        self.offline_since = Some(now);
        true
    }

    
    pub fn matches(&self, epoch: u64) -> bool {
        self.epoch == epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer() -> (Peer, tokio::sync::mpsc::UnboundedReceiver<crate::ws::Frame>) {
        let (outbox, receiver) = Outbox::new();
        let peer = Peer::new(1, [1u8; 32], "nick".to_string(), outbox, Instant::now());
        (peer, receiver)
    }

    #[test]
    fn fresh_peer_is_online() {
        let (peer, _receiver) = peer();
        assert!(peer.online());
        assert_eq!(peer.view().id, 1);
        assert!(peer.view().online);
    }

    #[test]
    fn detach_marks_peer_offline() {
        let (mut peer, _receiver) = peer();
        let now = Instant::now();

        assert!(peer.detach(peer.epoch(), now));
        assert!(!peer.online());
        assert_eq!(peer.offline_since(), Some(now));
    }

    #[test]
    fn stale_connection_does_not_break_the_new_one() {
        let (mut peer, _old) = peer();
        let stale = peer.epoch();

        let (outbox, _new) = Outbox::new();
        let fresh = peer.attach("nick".to_string(), outbox);

        assert_ne!(stale, fresh);
        assert!(!peer.detach(stale, Instant::now()));
        assert!(peer.online());
    }

    #[test]
    fn empty_nick_does_not_erase_the_old_one() {
        let (mut peer, _old) = peer();
        let (outbox, _new) = Outbox::new();
        peer.attach(String::new(), outbox);
        assert_eq!(peer.nick(), "nick");
    }
}
