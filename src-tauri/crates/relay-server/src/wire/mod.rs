pub mod client;
pub mod error;
pub mod server;

pub use client::{clean_nick, parse, ClientMessage};
pub use error::{ErrorCode, WireError};
pub use server::{PeerView, ServerMessage};

pub const PROTOCOL_VERSION: u8 = together_crypto::PROTOCOL_VERSION;

pub const NICK_LIMIT: usize = 512;
