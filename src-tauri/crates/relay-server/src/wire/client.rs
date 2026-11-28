use serde::Deserialize;
use serde_json::Value;

use super::error::{ErrorCode, WireError};
use super::NICK_LIMIT;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t")]
pub enum ClientMessage {
    
    #[serde(rename = "auth")]
    Auth {
        #[serde(rename = "pub")]
        public: String,
        #[serde(default)]
        v: Option<u8>,
    },
    
    #[serde(rename = "proof")]
    Proof { sig: String },
    
    #[serde(rename = "resume")]
    Resume { token: String },
    
    
    
    
    #[serde(rename = "room.create")]
    RoomCreate {
        #[serde(default)]
        ticket: Option<String>,
        #[serde(default)]
        nick: Option<String>,
    },
    
    #[serde(rename = "room.join")]
    RoomJoin {
        invite: String,
        #[serde(default)]
        nick: Option<String>,
    },
    
    #[serde(rename = "room.leave")]
    RoomLeave,
    
    #[serde(rename = "room.handoff")]
    RoomHandoff { to: u64 },
    
    #[serde(rename = "msg")]
    Msg { payload: Value },
    
    #[serde(rename = "ping")]
    Ping,
}

pub fn parse(text: &str) -> Result<ClientMessage, WireError> {
    let value: Value = serde_json::from_str(text).map_err(|_| WireError::new(ErrorCode::BadJson))?;
    serde_json::from_value(value).map_err(|_| WireError::new(ErrorCode::BadMessage))
}

pub fn clean_nick(nick: Option<String>) -> String {
    let raw = nick.unwrap_or_default();
    let trimmed = raw.trim();

    if trimmed.chars().count() <= NICK_LIMIT {
        return trimmed.to_string();
    }

    trimmed.chars().take(NICK_LIMIT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_messages_are_parsed() {
        let message = parse(r#"{"t":"auth","pub":"AAAA","v":1}"#).expect("auth разбирается");
        assert!(matches!(message, ClientMessage::Auth { .. }));

        let message = parse(r#"{"t":"proof","sig":"AAAA"}"#).expect("proof разбирается");
        assert!(matches!(message, ClientMessage::Proof { .. }));
    }

    #[test]
    fn room_messages_are_parsed() {
        let message = parse(r#"{"t":"room.create"}"#).expect("room.create разбирается");
        assert!(matches!(
            message,
            ClientMessage::RoomCreate {
                ticket: None,
                nick: None
            }
        ));

        let message = parse(r#"{"t":"room.handoff","to":3}"#).expect("handoff разбирается");
        assert!(matches!(message, ClientMessage::RoomHandoff { to: 3 }));

        let message = parse(r#"{"t":"room.leave"}"#).expect("leave разбирается");
        assert!(matches!(message, ClientMessage::RoomLeave));
    }

    #[test]
    fn payload_stays_untouched() {
        let message = parse(r#"{"t":"msg","payload":{"enc":"AAAA"}}"#).expect("msg разбирается");
        match message {
            ClientMessage::Msg { payload } => assert_eq!(payload["enc"], "AAAA"),
            other => panic!("ожидалось msg, пришло {other:?}"),
        }
    }

    #[test]
    fn broken_input_gives_different_codes() {
        assert_eq!(parse("{").unwrap_err().code, ErrorCode::BadJson);
        assert_eq!(
            parse(r#"{"t":"потанцуем"}"#).unwrap_err().code,
            ErrorCode::BadMessage
        );
        assert_eq!(parse(r#"{"t":"proof"}"#).unwrap_err().code, ErrorCode::BadMessage);
    }

    #[test]
    fn long_nick_is_cut() {
        let long = "я".repeat(NICK_LIMIT + 10);
        assert_eq!(clean_nick(Some(long)).chars().count(), NICK_LIMIT);
        assert_eq!(clean_nick(None), "");
    }
}
