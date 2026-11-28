use std::time::Instant;

use serde_json::Value;
use together_crypto::{invite as invite_codes, room_key};

use crate::wire::{clean_nick, ErrorCode, ServerMessage, WireError};
use crate::ws::{Context, Flow};

pub fn create(
    context: &mut Context,
    ticket: Option<String>,
    nick: Option<String>,
    now: Instant,
) -> Flow {
    let public = match context.require_identity() {
        Ok(public) => public,
        Err(flow) => return flow,
    };

    if context.room.is_some() {
        return context.refuse(ErrorCode::AlreadyInRoom);
    }

    let (id, invite) = match ticket {
        Some(ticket) => {
            if !room_key::is_ticket(&ticket) {
                return context.fail(WireError::with_text(
                    ErrorCode::BadTicket,
                    "билет комнаты должен быть 16 символов base32",
                ));
            }

            (ticket, None)
        }
        None => {
            if !context.state.config.privacy.allow_plain_invite {
                return context.fail(WireError::with_text(
                    ErrorCode::BadTicket,
                    "сервер принимает только готовые билеты комнат",
                ));
            }

            let invite = invite_codes::words();
            (room_key::ticket(&invite), Some(invite))
        }
    };

    let Some(lease) = context.state.ip_guard.acquire_room(context.ip) else {
        return context.fail(WireError::with_text(
            ErrorCode::IpLimit,
            "с этого адреса уже создано слишком много комнат",
        ));
    };

    let nick = clean_nick(nick);
    let outbox = context.outbox.clone();

    let joined = match context
        .state
        .rooms
        .create(id, invite.clone(), lease, public, nick.clone(), outbox, now)
    {
        Ok(joined) => joined,
        Err(code) => return context.refuse(code),
    };

    context.enter(joined.room.clone(), joined.seat, nick);

    
    context.send(&ServerMessage::Room {
        id: joined.room.clone(),
        invite,
        host: joined.host,
    });

    context.state.rooms.greet(&joined.room, joined.seat);
    Flow::Continue
}

pub fn join(context: &mut Context, invite: &str, nick: Option<String>, now: Instant) -> Flow {
    let public = match context.require_identity() {
        Ok(public) => public,
        Err(flow) => return flow,
    };

    if context.room.is_some() {
        return context.refuse(ErrorCode::AlreadyInRoom);
    }

    let id = if room_key::is_ticket(invite) {
        invite.to_string()
    } else {
        if !context.state.config.privacy.allow_plain_invite {
            return context.fail(WireError::with_text(
                ErrorCode::BadTicket,
                "сервер принимает только готовые билеты комнат",
            ));
        }

        if !invite_codes::looks_valid(invite) {
            return context.fail(WireError::with_text(
                ErrorCode::BadTicket,
                "код приглашения не похож на настоящий",
            ));
        }

        room_key::ticket(invite)
    };

    let nick = clean_nick(nick);
    let outbox = context.outbox.clone();

    let joined = match context
        .state
        .rooms
        .join(&id, public, nick.clone(), outbox, now)
    {
        Ok(joined) => joined,
        Err(code) => return context.refuse(code),
    };

    context.enter(joined.room.clone(), joined.seat, nick);
    context.state.rooms.greet(&joined.room, joined.seat);
    Flow::Continue
}

pub fn leave(context: &mut Context, now: Instant) -> Flow {
    let (room, seat) = match context.require_room() {
        Ok(pair) => pair,
        Err(flow) => return flow,
    };

    context.state.rooms.leave(&room, seat, now);
    context.forget_room();
    Flow::Continue
}

pub fn handoff(context: &mut Context, to: u64) -> Flow {
    let (room, seat) = match context.require_room() {
        Ok(pair) => pair,
        Err(flow) => return flow,
    };

    match context.state.rooms.handoff(&room, seat, to) {
        Ok(_) => Flow::Continue,
        Err(code) => context.refuse(code),
    }
}

pub fn relay(context: &mut Context, payload: Value) -> Flow {
    let (room, seat) = match context.require_room() {
        Ok(pair) => pair,
        Err(flow) => return flow,
    };

    match context.state.rooms.relay(&room, seat, payload) {
        Ok(_) => Flow::Continue,
        Err(code) => context.refuse(code),
    }
}
