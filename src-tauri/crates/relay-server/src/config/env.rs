use std::str::FromStr;

use super::{Config, ConfigError};

const PREFIX: &str = "MASHIRO_TOGETHER_";

pub fn apply(config: &mut Config) -> Result<(), ConfigError> {
    if let Some(value) = text("ADDR") {
        config.server.address = value;
    }
    if let Some(value) = parse::<u16>("PORT")? {
        config.server.port = value;
    }
    if let Some(value) = text("LOG_LEVEL") {
        config.log.level = value;
    }
    if let Some(value) = parse::<bool>("METRICS")? {
        config.metrics.enabled = value;
    }
    if let Some(value) = parse::<bool>("TRUST_PROXY")? {
        config.privacy.trust_proxy = value;
    }
    if let Some(value) = parse::<bool>("ALLOW_PLAIN_INVITE")? {
        config.privacy.allow_plain_invite = value;
    }
    if let Some(value) = parse::<usize>("MAX_PEERS")? {
        config.limits.max_peers_per_room = value;
    }
    if let Some(value) = parse::<u32>("MSG_RATE")? {
        config.limits.messages_per_second = value;
    }
    if let Some(value) = parse::<u32>("STRIKES")? {
        config.limits.strikes_before_close = value;
    }
    if let Some(value) = parse::<usize>("MAX_MESSAGE_BYTES")? {
        config.limits.max_message_bytes = value;
    }
    if let Some(value) = parse::<usize>("MAX_CONNS_PER_IP")? {
        config.limits.max_connections_per_ip = value;
    }
    if let Some(value) = parse::<usize>("MAX_ROOMS_PER_IP")? {
        config.limits.max_rooms_per_ip = value;
    }
    if let Some(value) = parse::<u64>("PING_SECONDS")? {
        config.limits.ping_seconds = value;
    }
    if let Some(value) = parse::<u64>("PONG_TIMEOUT")? {
        config.limits.pong_timeout_seconds = value;
    }
    if let Some(value) = parse::<u64>("SESSION_TTL")? {
        config.limits.session_ttl_seconds = value;
    }
    if let Some(value) = parse::<u64>("CHALLENGE_TTL")? {
        config.limits.challenge_ttl_seconds = value;
    }
    if let Some(value) = parse::<u64>("HOST_GRACE")? {
        config.limits.host_grace_seconds = value;
    }
    if let Some(value) = parse::<u64>("EMPTY_ROOM_TTL")? {
        config.limits.empty_room_ttl_seconds = value;
    }

    Ok(())
}

fn text(name: &str) -> Option<String> {
    let value = std::env::var(format!("{PREFIX}{name}")).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse<T: FromStr>(name: &str) -> Result<Option<T>, ConfigError> {
    let Some(value) = text(name) else {
        return Ok(None);
    };

    value.parse::<T>().map(Some).map_err(|_| ConfigError::Env {
        name: format!("{PREFIX}{name}"),
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_wins_over_file() {
        
        std::env::set_var("MASHIRO_TOGETHER_MAX_PEERS", "4");
        let mut config = Config::default();
        apply(&mut config).expect("переменные разобраны");
        std::env::remove_var("MASHIRO_TOGETHER_MAX_PEERS");

        assert_eq!(config.limits.max_peers_per_room, 4);
    }

    #[test]
    fn garbage_in_env_is_an_error() {
        std::env::set_var("MASHIRO_TOGETHER_PING_SECONDS", "пять");
        let mut config = Config::default();
        let result = apply(&mut config);
        std::env::remove_var("MASHIRO_TOGETHER_PING_SECONDS");

        assert!(result.is_err());
    }
}
