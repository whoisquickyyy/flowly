use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::app::Shared;

pub async fn handler(State(state): State<Shared>) -> Json<Value> {
    let snapshot = state.counters.snapshot();
    let status = if state.stopping() { "stopping" } else { "ok" };

    Json(json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.uptime().as_secs(),
        "rooms_open": snapshot.rooms_open,
        "connections_open": snapshot.connections_open,
    }))
}
