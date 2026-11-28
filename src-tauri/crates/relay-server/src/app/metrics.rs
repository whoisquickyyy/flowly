use axum::extract::State;
use axum::http::header::{HeaderValue, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};

use crate::app::Shared;
use crate::telemetry::counters::Snapshot;

pub const CONTENT: &str = "text/plain; version=0.0.4; charset=utf-8";

pub async fn handler(State(state): State<Shared>) -> Response {
    let body = render(&state.counters.snapshot(), state.uptime().as_secs());

    (
        [(CONTENT_TYPE, HeaderValue::from_static(CONTENT))],
        body,
    )
        .into_response()
}

fn render(snapshot: &Snapshot, uptime: u64) -> String {
    let mut text = String::new();

    metric(
        &mut text,
        "mashiro_together_uptime_seconds",
        "counter",
        "Сколько секунд работает процесс",
        uptime,
    );
    metric(
        &mut text,
        "mashiro_together_connections_total",
        "counter",
        "Сколько соединений было с момента старта",
        snapshot.connections_total,
    );
    metric(
        &mut text,
        "mashiro_together_connections_open",
        "gauge",
        "Сколько соединений сейчас открыто",
        snapshot.connections_open,
    );
    metric(
        &mut text,
        "mashiro_together_rooms_total",
        "counter",
        "Сколько комнат было создано",
        snapshot.rooms_total,
    );
    metric(
        &mut text,
        "mashiro_together_rooms_open",
        "gauge",
        "Сколько комнат живо сейчас",
        snapshot.rooms_open,
    );
    metric(
        &mut text,
        "mashiro_together_messages_relayed_total",
        "counter",
        "Сколько раз сообщение было доставлено участнику",
        snapshot.messages_relayed,
    );
    metric(
        &mut text,
        "mashiro_together_handshakes_failed_total",
        "counter",
        "Сколько рукопожатий не сошлось",
        snapshot.handshakes_failed,
    );
    metric(
        &mut text,
        "mashiro_together_rate_limited_total",
        "counter",
        "Сколько раз сработал ограничитель скорости",
        snapshot.rate_limited,
    );
    metric(
        &mut text,
        "mashiro_together_errors_total",
        "counter",
        "Сколько ошибок было отправлено клиентам",
        snapshot.errors_total,
    );

    text
}

fn metric(text: &mut String, name: &str, kind: &str, help: &str, value: u64) {
    text.push_str(&format!("# HELP {name} {help}\n"));
    text.push_str(&format!("# TYPE {name} {kind}\n"));
    text.push_str(&format!("{name} {value}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            connections_total: 7,
            connections_open: 3,
            rooms_total: 2,
            rooms_open: 1,
            messages_relayed: 900,
            handshakes_failed: 4,
            rate_limited: 5,
            errors_total: 6,
        }
    }

    #[test]
    fn exposition_has_values_and_types() {
        let text = render(&snapshot(), 42);

        assert!(text.contains("# TYPE mashiro_together_rooms_open gauge"));
        assert!(text.contains("mashiro_together_rooms_open 1\n"));
        assert!(text.contains("mashiro_together_messages_relayed_total 900\n"));
        assert!(text.contains("mashiro_together_uptime_seconds 42\n"));
    }

    #[test]
    fn exposition_has_no_labels() {
        let text = render(&snapshot(), 1);
        assert!(!text.contains('{'));
    }
}
