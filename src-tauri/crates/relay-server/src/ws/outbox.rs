use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::wire::{ServerMessage, WireError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    
    Text(String),
    
    Close,
}

#[derive(Debug, Clone)]
pub struct Outbox {
    sender: UnboundedSender<Frame>,
}

impl Outbox {
    
    pub fn new() -> (Self, UnboundedReceiver<Frame>) {
        let (sender, receiver) = unbounded_channel();
        (Self { sender }, receiver)
    }

    
    pub fn send(&self, message: &ServerMessage) -> bool {
        self.sender.send(Frame::Text(message.to_frame())).is_ok()
    }

    
    pub fn send_error(&self, error: &WireError) -> bool {
        self.send(&ServerMessage::error(error))
    }

    
    pub fn close(&self) {
        let _ = self.sender.send(Frame::Close);
    }

    
    pub fn is_open(&self) -> bool {
        !self.sender.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::ErrorCode;

    #[test]
    fn message_reaches_the_writer() {
        let (outbox, mut receiver) = Outbox::new();
        assert!(outbox.send(&ServerMessage::Pong));

        match receiver.try_recv() {
            Ok(Frame::Text(text)) => assert!(text.contains("pong")),
            other => panic!("ожидался текст, пришло {other:?}"),
        }
    }

    #[test]
    fn error_is_serialized() {
        let (outbox, mut receiver) = Outbox::new();
        outbox.send_error(&WireError::new(ErrorCode::NoRoom));

        match receiver.try_recv() {
            Ok(Frame::Text(text)) => assert!(text.contains("no_room")),
            other => panic!("ожидалась ошибка, пришло {other:?}"),
        }
    }

    #[test]
    fn dead_receiver_is_visible() {
        let (outbox, receiver) = Outbox::new();
        drop(receiver);

        assert!(!outbox.is_open());
        assert!(!outbox.send(&ServerMessage::Pong));
    }
}
