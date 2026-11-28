use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::app::{forwarded, health, metrics, Shared};
use crate::ws::connection;

const SIZE_SLACK: usize = 4 * 1024;

pub fn build(state: Shared) -> Router {
    let mut router = Router::new()
        .route("/health", get(health::handler))
        .route("/ws", get(upgrade));

    if state.config.metrics.enabled {
        router = router.route("/metrics", get(metrics::handler));
    }

    router.with_state(state)
}

async fn upgrade(
    State(state): State<Shared>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let address = forwarded::client_ip(&headers, remote.ip(), state.config.privacy.trust_proxy);
    let ip = state.ip_guard.key(address);

    let max_bytes = state
        .config
        .limits
        .max_message_bytes
        .saturating_add(SIZE_SLACK);

    upgrade
        .max_message_size(max_bytes)
        .max_frame_size(max_bytes)
        .on_upgrade(move |socket| connection::run(socket, state, ip))
}
