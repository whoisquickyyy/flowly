use std::time::Instant;

use crate::app::Shared;
use crate::auth::Challenge;
use crate::limits::{ConnectionLease, IpKey, RateLimiter, Verdict};
use crate::rooms::Seat;
use crate::wire::{self, ClientMessage, ErrorCode, ServerMessage, WireError};
use crate::ws::{handshake, rooms, Outbox};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    
    Continue,
    
    Close,
}

pub struct Context {
    
    pub state: Shared,
    
    pub outbox: Outbox,
    
    pub ip: IpKey,
    
    pub rate: RateLimiter,
    
    pub identity: Option<[u8; 32]>,
    
    pub challenge: Option<Challenge>,
    
    pub room: Option<String>,
    
    pub seat: Option<Seat>,
    
    pub nick: String,
    
    _lease: ConnectionLease,
}

impl Context {
    pub fn new(state: Shared, outbox: Outbox, ip: IpKey, lease: ConnectionLease) -> Self {
        let rate = RateLimiter::new(&state.config.limits);

        Self {
            state,
            outbox,
            ip,
            rate,
            identity: None,
            challenge: None,
            room: None,
            seat: None,
            nick: String::new(),
            _lease: lease,
        }
    }

    
    pub fn handle_text(&mut self, text: &str, now: Instant) -> Flow {
        match self.rate.check_at(now) {
            Verdict::Allow => {}
            Verdict::Reject => {
                self.state.counters.rate_limited();
                return self.fail(WireError::new(ErrorCode::RateLimit));
            }
            Verdict::Close => {
                self.state.counters.rate_limited();
                self.fail(WireError::with_text(
                    ErrorCode::RateLimit,
                    "поток сообщений не прекращается, соединение закрыто",
                ));

                return Flow::Close;
            }
        }

        match wire::parse(text) {
            Ok(message) => self.dispatch(message, now),
            Err(error) => self.fail(error),
        }
    }

    
    pub fn dispatch(&mut self, message: ClientMessage, now: Instant) -> Flow {
        match message {
            ClientMessage::Auth { public, v } => handshake::auth(self, &public, v),
            ClientMessage::Proof { sig } => handshake::proof(self, &sig, now),
            ClientMessage::Resume { token } => handshake::resume(self, &token, now),
            ClientMessage::RoomCreate { ticket, nick } => rooms::create(self, ticket, nick, now),
            ClientMessage::RoomJoin { invite, nick } => rooms::join(self, &invite, nick, now),
            ClientMessage::RoomLeave => rooms::leave(self, now),
            ClientMessage::RoomHandoff { to } => rooms::handoff(self, to),
            ClientMessage::Msg { payload } => rooms::relay(self, payload),
            ClientMessage::Ping => {
                self.send(&ServerMessage::Pong);
                Flow::Continue
            }
        }
    }

    
    pub fn send(&self, message: &ServerMessage) {
        self.outbox.send(message);
    }

    
    pub fn fail(&self, error: WireError) -> Flow {
        self.state.counters.error_sent();
        self.outbox.send_error(&error);

        if error.is_fatal() {
            Flow::Close
        } else {
            Flow::Continue
        }
    }

    
    pub fn refuse(&self, code: ErrorCode) -> Flow {
        self.fail(WireError::new(code))
    }

    
    pub fn require_identity(&self) -> Result<[u8; 32], Flow> {
        self.identity.ok_or_else(|| self.refuse(ErrorCode::Unauthorized))
    }

    
    pub fn require_room(&self) -> Result<(String, Seat), Flow> {
        match (self.room.clone(), self.seat) {
            (Some(room), Some(seat)) => Ok((room, seat)),
            _ => Err(self.refuse(ErrorCode::NoRoom)),
        }
    }

    
    pub fn enter(&mut self, room: String, seat: Seat, nick: String) {
        self.room = Some(room);
        self.seat = Some(seat);
        self.nick = nick;
    }

    
    pub fn forget_room(&mut self) {
        self.room = None;
        self.seat = None;
    }

    
    
    
    
    pub fn on_disconnect(&mut self, now: Instant) {
        if let (Some(room), Some(seat)) = (self.room.as_deref(), self.seat) {
            self.state.rooms.detach(room, seat, now);
        }

        self.forget_room();
    }
}
