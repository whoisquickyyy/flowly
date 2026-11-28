use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    
    RateLimit,
    
    BadSig,
    
    NoRoom,
    
    Full,
    
    BadJson,
    
    BadMessage,
    
    TooLarge,
    
    Unauthorized,
    
    ChallengeExpired,
    
    NotHost,
    
    NoPeer,
    
    AlreadyInRoom,
    
    BadTicket,
    
    IpLimit,
    
    Internal,
}

impl ErrorCode {
    
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RateLimit => "rate_limit",
            Self::BadSig => "bad_sig",
            Self::NoRoom => "no_room",
            Self::Full => "full",
            Self::BadJson => "bad_json",
            Self::BadMessage => "bad_message",
            Self::TooLarge => "too_large",
            Self::Unauthorized => "unauthorized",
            Self::ChallengeExpired => "challenge_expired",
            Self::NotHost => "not_host",
            Self::NoPeer => "no_peer",
            Self::AlreadyInRoom => "already_in_room",
            Self::BadTicket => "bad_ticket",
            Self::IpLimit => "ip_limit",
            Self::Internal => "internal",
        }
    }

    
    pub fn text(self) -> &'static str {
        match self {
            Self::RateLimit => "слишком частые сообщения",
            Self::BadSig => "подпись не прошла проверку",
            Self::NoRoom => "комната не найдена",
            Self::Full => "в комнате нет свободных мест",
            Self::BadJson => "сообщение не разобралось",
            Self::BadMessage => "неизвестное сообщение",
            Self::TooLarge => "сообщение слишком большое",
            Self::Unauthorized => "сначала нужно пройти рукопожатие",
            Self::ChallengeExpired => "время на ответ истекло",
            Self::NotHost => "так может только хост комнаты",
            Self::NoPeer => "участник не найден",
            Self::AlreadyInRoom => "соединение уже в комнате",
            Self::BadTicket => "код комнаты не подходит",
            Self::IpLimit => "с этого адреса слишком много подключений",
            Self::Internal => "внутренняя ошибка",
        }
    }

    
    pub fn is_fatal(self) -> bool {
        matches!(self, Self::TooLarge)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireError {
    pub code: ErrorCode,
    pub text: String,
}

impl WireError {
    
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            text: code.text().to_string(),
        }
    }

    
    pub fn with_text(code: ErrorCode, text: impl Into<String>) -> Self {
        Self {
            code,
            text: text.into(),
        }
    }

    pub fn is_fatal(&self) -> bool {
        self.code.is_fatal()
    }
}

impl From<ErrorCode> for WireError {
    fn from(code: ErrorCode) -> Self {
        Self::new(code)
    }
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.text)
    }
}

impl std::error::Error for WireError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_codes_keep_their_names() {
        assert_eq!(ErrorCode::RateLimit.as_str(), "rate_limit");
        assert_eq!(ErrorCode::BadSig.as_str(), "bad_sig");
        assert_eq!(ErrorCode::NoRoom.as_str(), "no_room");
        assert_eq!(ErrorCode::Full.as_str(), "full");
    }

    #[test]
    fn only_hard_errors_close_connection() {
        assert!(ErrorCode::TooLarge.is_fatal());
        assert!(!ErrorCode::RateLimit.is_fatal());
        assert!(!ErrorCode::NoRoom.is_fatal());
    }
}
