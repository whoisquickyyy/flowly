use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{interval, timeout, MissedTickBehavior};

use crate::app::Shared;
use crate::limits::IpKey;
use crate::wire::{ErrorCode, ServerMessage, WireError};
use crate::ws::{Context, Flow, Frame, Outbox};

pub async fn run(socket: WebSocket, state: Shared, ip: IpKey) {
    let Some(lease) = state.ip_guard.acquire_connection(ip) else {
        refuse(
            socket,
            &WireError::with_text(
                ErrorCode::IpLimit,
                "с этого адреса уже слишком много соединений",
            ),
        )
        .await;

        return;
    };

    state.counters.connection_opened();

    let ping = state.config.limits.ping_interval();
    let (outbox, queue) = Outbox::new();
    let (sink, stream) = socket.split();
    let writer = tokio::spawn(write_loop(sink, queue, ping));

    let mut context = Context::new(Arc::clone(&state), outbox.clone(), ip, lease);
    read_loop(stream, &mut context).await;
    context.on_disconnect(Instant::now());
    drop(context);

    outbox.close();
    let _ = writer.await;
    state.counters.connection_closed();
}

async fn read_loop(mut stream: SplitStream<WebSocket>, context: &mut Context) {
    let silence = context.state.config.limits.pong_timeout();
    let max_bytes = context.state.config.limits.max_message_bytes;
    let mut stop = context.state.shutdown_watch();

    loop {
        let received = tokio::select! {
            _ = stop.changed() => break,
            received = timeout(silence, stream.next()) => received,
        };

        let message = match received {
            
            Err(_) => break,
            Ok(None) => break,
            Ok(Some(Err(_))) => break,
            Ok(Some(Ok(message))) => message,
        };

        let flow = match message {
            Message::Text(text) => {
                if text.len() > max_bytes {
                    context.fail(WireError::with_text(
                        ErrorCode::TooLarge,
                        "сообщение больше разрешённого размера",
                    ))
                } else {
                    context.handle_text(&text, Instant::now())
                }
            }
            Message::Binary(_) => context.fail(WireError::with_text(
                ErrorCode::BadMessage,
                "сервер принимает только текстовые кадры",
            )),
            Message::Close(_) => break,
            
            Message::Ping(_) | Message::Pong(_) => Flow::Continue,
        };

        if flow == Flow::Close {
            break;
        }
    }
}

async fn write_loop(
    mut sink: SplitSink<WebSocket, Message>,
    mut queue: UnboundedReceiver<Frame>,
    ping: Duration,
) {
    let mut ticker = interval(ping);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ticker.tick().await;

    loop {
        let outgoing = tokio::select! {
            frame = queue.recv() => match frame {
                Some(Frame::Text(text)) => Message::Text(text),
                Some(Frame::Close) | None => break,
            },
            _ = ticker.tick() => Message::Ping(Vec::new()),
        };

        if sink.send(outgoing).await.is_err() {
            break;
        }
    }

    let _ = sink.close().await;
}

async fn refuse(mut socket: WebSocket, error: &WireError) {
    let text = ServerMessage::error(error).to_frame();
    let _ = socket.send(Message::Text(text)).await;
    let _ = SinkExt::close(&mut socket).await;
}
