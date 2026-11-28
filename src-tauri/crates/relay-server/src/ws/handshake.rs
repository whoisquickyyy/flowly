use std::time::Instant;

use crate::auth::{identity, Challenge};
use crate::wire::{ErrorCode, ServerMessage, WireError, PROTOCOL_VERSION};
use crate::ws::{Context, Flow};

pub fn auth(context: &mut Context, public: &str, version: Option<u8>) -> Flow {
    if version.is_some_and(|value| value != PROTOCOL_VERSION) {
        return context.fail(WireError::with_text(
            ErrorCode::BadMessage,
            "версия протокола не поддерживается",
        ));
    }

    let Some(key) = identity::public_from_base64(public) else {
        context.state.counters.handshake_failed();
        return context.fail(WireError::with_text(
            ErrorCode::BadSig,
            "публичный ключ не разобрался",
        ));
    };

    let challenge = Challenge::issue(key, context.state.config.limits.challenge_ttl());
    context.send(&ServerMessage::Challenge {
        nonce: challenge.nonce_base64(),
    });

    context.challenge = Some(challenge);
    Flow::Continue
}

pub fn proof(context: &mut Context, signature: &str, now: Instant) -> Flow {
    let Some(challenge) = context.challenge.take() else {
        context.state.counters.handshake_failed();
        return context.refuse(ErrorCode::ChallengeExpired);
    };

    let Some(signature) = identity::signature_from_base64(signature) else {
        context.state.counters.handshake_failed();
        return context.fail(WireError::with_text(
            ErrorCode::BadSig,
            "подпись не разобралась",
        ));
    };

    match challenge.verify(&signature, now) {
        Ok(public) => {
            let bytes = public.to_bytes();
            context.identity = Some(bytes);
            grant(context, bytes, now);
            Flow::Continue
        }
        Err(code) => {
            context.state.counters.handshake_failed();
            context.refuse(code)
        }
    }
}

pub fn resume(context: &mut Context, token: &str, now: Instant) -> Flow {
    let Some(public) = context.state.sessions.resume(token, now) else {
        context.state.counters.handshake_failed();
        return context.fail(WireError::with_text(
            ErrorCode::Unauthorized,
            "токен не подходит или истёк",
        ));
    };

    context.identity = Some(public);
    grant(context, public, now);
    Flow::Continue
}

fn grant(context: &Context, public: [u8; 32], now: Instant) {
    let limits = &context.state.config.limits;
    let token = context
        .state
        .sessions
        .issue_at(public, limits.session_ttl(), now);

    context.send(&ServerMessage::Session {
        token,
        ttl: limits.session_ttl_seconds,
    });
}
