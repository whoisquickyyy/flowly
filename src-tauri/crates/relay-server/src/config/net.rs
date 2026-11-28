use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerSection {
    pub address: String,
    pub port: u16,
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_string(),
            port: 7332,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PrivacySection {
    
    pub trust_proxy: bool,
    
    
    
    
    pub allow_plain_invite: bool,
}

impl Default for PrivacySection {
    fn default() -> Self {
        Self {
            trust_proxy: true,
            allow_plain_invite: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LogSection {
    pub level: String,
}

impl Default for LogSection {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MetricsSection {
    pub enabled: bool,
}
