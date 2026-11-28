use serde::Serialize;
use serde_json::Value;

use super::error::WireError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PeerView {
    pub id: u64,
    pub nick: String,
    
    pub online: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t")]
pub enum ServerMessage {
    
    #[serde(rename = "challenge")]
    Challenge { nonce: String },
    
    #[serde(rename = "session")]
    Session { token: String, ttl: u64 },
    
    #[serde(rename = "room")]
    Room {
        id: String,
        
        #[serde(skip_serializing_if = "Option::is_none")]
        invite: Option<String>,
        host: u64,
    },
    
    #[serde(rename = "welcome")]
    Welcome { id: u64, room: String, host: u64 },
    
    #[serde(rename = "peers")]
    Peers { peers: Vec<PeerView>, host: u64 },
    
    #[serde(rename = "host")]
    Host { id: u64 },
    
    #[serde(rename = "msg")]
    Msg {
        from: u64,
        nick: String,
        payload: Value,
    },
    
    #[serde(rename = "pong")]
    Pong,
    
    #[serde(rename = "error")]
    Error { code: String, text: String },
}

impl ServerMessage {
    
    pub fn error(error: &WireError) -> Self {
        Self::Error {
            code: error.code.as_str().to_string(),
            text: error.text.clone(),
        }
    }

    
    
    
    
    pub fn to_frame(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"t":"error","code":"internal","text":"внутренняя ошибка"}"#.to_string()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::error::ErrorCode;

    #[test]
    fn welcome_keeps_field_names() {
        let frame = ServerMessage::Welcome {
            id: 2,
            room: "abcdefghij234567".to_string(),
            host: 1,
        }
        .to_frame();

        assert!(frame.contains(r#""t":"welcome""#));
        assert!(frame.contains(r#""id":2"#));
        assert!(frame.contains(r#""host":1"#));
    }

    #[test]
    fn room_without_invite_hides_the_field() {
        let frame = ServerMessage::Room {
            id: "abcdefghij234567".to_string(),
            invite: None,
            host: 1,
        }
        .to_frame();

        assert!(!frame.contains("invite"));
    }

    #[test]
    fn error_carries_code_and_text() {
        let frame = ServerMessage::error(&WireError::new(ErrorCode::Full)).to_frame();
        assert!(frame.contains(r#""code":"full""#));
        assert!(frame.contains(r#""t":"error""#));
    }

    #[test]
    fn peers_list_is_an_array() {
        let frame = ServerMessage::Peers {
            peers: vec![PeerView {
                id: 1,
                nick: "AAAA".to_string(),
                online: true,
            }],
            host: 1,
        }
        .to_frame();

        assert!(frame.contains(r#""peers":[{"id":1,"nick":"AAAA","online":true}]"#));
    }
}
