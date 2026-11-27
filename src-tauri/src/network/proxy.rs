use std::sync::{Arc, OnceLock};
use std::time::Duration;
use reqwest::{Client, Proxy};
use thiserror::Error;

use crate::models::{ProxyConfig, ProxyMode};
use crate::network::doh::DohResolver;

static SHARED_DOH: OnceLock<Arc<DohResolver>> = OnceLock::new();

pub fn shared_doh() -> Arc<DohResolver> {
    Arc::clone(SHARED_DOH.get_or_init(|| Arc::new(DohResolver::new())))
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Ошибка сборки HTTP-клиента: {0}")]
    ClientBuild(#[from] reqwest::Error),
    #[error("Некорректный адрес прокси: {0}")]
    InvalidProxy(String),
}

/// Собирает изолированный HTTP-клиент для конкретного музыкального источника.
/// Для каждого провайдера используется свой экземпляр, что обеспечивает split-tunneling:
/// Например, Яндекс Музыка ходит напрямую (Direct), а SoundCloud — через SOCKS5.
pub fn build_client(config: &ProxyConfig) -> Result<Client, NetworkError> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .http1_only()
        .user_agent("FlowlyMusicPlayer/0.1.0");

    match config.mode {
        ProxyMode::Direct => {
            // Явно отключаем системный прокси и используем встроенный DoH (1.1.1.1 / 8.8.8.8)
            builder = builder.no_proxy().dns_resolver(shared_doh());
        }
        ProxyMode::Socks5 | ProxyMode::Http => {
            if config.host.trim().is_empty() || config.port == 0 {
                return Err(NetworkError::InvalidProxy(
                    "Укажите хост и порт прокси".to_string(),
                ));
            }
            // Используем socks5h:// чтобы DNS-запросы также шли через прокси (защита от DNS-блокировок)
            let scheme = match config.mode {
                ProxyMode::Socks5 => "socks5h",
                ProxyMode::Http => "http",
                ProxyMode::Direct => unreachable!(),
            };
            let proxy_url = format!("{}://{}:{}", scheme, config.host.trim(), config.port);
            let mut proxy = Proxy::all(&proxy_url)?;

            if let (Some(user), Some(pass)) = (&config.username, &config.password) {
                if !user.is_empty() {
                    proxy = proxy.basic_auth(user, pass);
                }
            }
            builder = builder.proxy(proxy);
        }
    }

    Ok(builder.build()?)
}
