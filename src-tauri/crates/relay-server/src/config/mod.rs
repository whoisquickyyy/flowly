mod env;
mod limits;
mod net;

use std::fmt;
use std::fs;
use std::path::Path;

use serde::Deserialize;

pub use limits::Limits;
pub use net::{LogSection, MetricsSection, PrivacySection, ServerSection};

pub const DEFAULT_PATH: &str = "config.toml";

pub const PATH_ENV: &str = "MASHIRO_TOGETHER_CONFIG";

#[derive(Debug)]
pub enum ConfigError {
    
    Read { path: String, reason: String },
    
    Parse { path: String, reason: String },
    
    Env { name: String, value: String },
    
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, reason } => write!(formatter, "файл {path} не прочитан: {reason}"),
            Self::Parse { path, reason } => write!(formatter, "файл {path} не разобран: {reason}"),
            Self::Env { name, value } => {
                write!(formatter, "переменная {name} содержит неверное значение: {value}")
            }
            Self::Invalid(text) => formatter.write_str(text),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub server: ServerSection,
    pub limits: Limits,
    pub privacy: PrivacySection,
    pub log: LogSection,
    pub metrics: MetricsSection,
}

impl Config {
    
    
    
    pub fn load() -> Result<Self, ConfigError> {
        let explicit = std::env::var(PATH_ENV).ok();
        let path = explicit.clone().unwrap_or_else(|| DEFAULT_PATH.to_string());

        let mut config = if Path::new(&path).exists() {
            Self::from_file(&path)?
        } else if explicit.is_some() {
            return Err(ConfigError::Read {
                path,
                reason: "файл не найден".to_string(),
            });
        } else {
            Self::default()
        };

        env::apply(&mut config)?;
        config.check()?;
        Ok(config)
    }

    
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|error| ConfigError::Read {
            path: path.to_string(),
            reason: error.to_string(),
        })?;

        toml::from_str(&text).map_err(|error| ConfigError::Parse {
            path: path.to_string(),
            reason: error.to_string(),
        })
    }

    
    fn check(&self) -> Result<(), ConfigError> {
        if self.limits.max_peers_per_room == 0 {
            return Err(ConfigError::Invalid(
                "в комнате должен помещаться хотя бы один участник".to_string(),
            ));
        }

        if self.limits.messages_per_second == 0 {
            return Err(ConfigError::Invalid(
                "лимит сообщений в секунду не может быть нулём".to_string(),
            ));
        }

        if self.limits.max_message_bytes < 1024 {
            return Err(ConfigError::Invalid(
                "размер сообщения меньше килобайта сделает комнаты нерабочими".to_string(),
            ));
        }

        if self.limits.pong_timeout_seconds <= self.limits.ping_seconds {
            return Err(ConfigError::Invalid(
                "таймаут pong должен быть больше интервала ping".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_documentation() {
        let config = Config::default();
        assert_eq!(config.server.port, 7332);
        assert_eq!(config.limits.max_peers_per_room, 16);
        assert_eq!(config.limits.messages_per_second, 20);
        assert_eq!(config.limits.max_message_bytes, 256 * 1024);
        assert_eq!(config.limits.host_grace_seconds, 15);
        assert_eq!(config.limits.empty_room_ttl_seconds, 600);
        assert!(!config.metrics.enabled);
    }

    #[test]
    fn toml_overrides_only_given_fields() {
        let config: Config = toml::from_str("[server]\nport = 9000\n").expect("toml разбирается");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.limits.max_peers_per_room, 16);
    }

    #[test]
    fn broken_limits_are_rejected() {
        let mut config = Config::default();
        config.limits.max_peers_per_room = 0;
        assert!(config.check().is_err());

        let mut config = Config::default();
        config.limits.pong_timeout_seconds = 5;
        config.limits.ping_seconds = 15;
        assert!(config.check().is_err());
    }
}
