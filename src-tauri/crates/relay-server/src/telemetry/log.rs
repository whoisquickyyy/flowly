use std::sync::Once;

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

static READY: Once = Once::new();

pub fn init(level: &str) {
    READY.call_once(|| {
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse(level)
            .unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_ansi(false)
            .init();
    });
}
