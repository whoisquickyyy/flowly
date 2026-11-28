use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Limits {
    
    pub max_peers_per_room: usize,
    
    pub messages_per_second: u32,
    
    pub strikes_before_close: u32,
    
    pub max_message_bytes: usize,
    
    pub max_connections_per_ip: usize,
    
    pub max_rooms_per_ip: usize,
    
    pub ping_seconds: u64,
    
    pub pong_timeout_seconds: u64,
    
    pub session_ttl_seconds: u64,
    
    pub challenge_ttl_seconds: u64,
    
    pub host_grace_seconds: u64,
    
    pub empty_room_ttl_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_peers_per_room: 16,
            messages_per_second: 20,
            strikes_before_close: 40,
            max_message_bytes: 256 * 1024,
            max_connections_per_ip: 8,
            max_rooms_per_ip: 4,
            ping_seconds: 15,
            pong_timeout_seconds: 30,
            session_ttl_seconds: 3600,
            challenge_ttl_seconds: 10,
            host_grace_seconds: 15,
            empty_room_ttl_seconds: 600,
        }
    }
}

impl Limits {
    pub fn ping_interval(&self) -> Duration {
        Duration::from_secs(self.ping_seconds)
    }

    pub fn pong_timeout(&self) -> Duration {
        Duration::from_secs(self.pong_timeout_seconds)
    }

    pub fn session_ttl(&self) -> Duration {
        Duration::from_secs(self.session_ttl_seconds)
    }

    pub fn challenge_ttl(&self) -> Duration {
        Duration::from_secs(self.challenge_ttl_seconds)
    }

    pub fn host_grace(&self) -> Duration {
        Duration::from_secs(self.host_grace_seconds)
    }

    pub fn empty_room_ttl(&self) -> Duration {
        Duration::from_secs(self.empty_room_ttl_seconds)
    }
}
