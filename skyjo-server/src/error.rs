use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Server error type replacing String errors throughout the codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "detail")]
pub enum ServerError {
    // Room errors
    RoomNotFound,
    RoomFull,
    RoomCodeInvalid,
    MaxRoomsReached,

    // Slot/player errors
    InvalidSlot(usize),
    SlotEmpty,
    SlotOccupied,
    CannotModifyCreator,
    CannotBanSameIp,

    // Game state errors
    NotInLobby,
    NotInGame,
    NotYourTurn,
    GameNotStarted,
    GameAlreadyStarted,
    NotAllSlotsFilled,

    // Action errors
    InvalidAction(String),
    InvalidPosition(usize),

    // Permission errors
    NotHost,
    Unauthorized,
    Banned,

    // Rate limiting
    RateLimited,

    // Validation errors
    PlayerNameTooLong,
    PlayerNameEmpty,
    InvalidTurnTimer,
    InvalidDisconnectTimeout,
    InvalidNumPlayers,
    InvalidStrategy(String),
    InvalidRules(String),

    // Internal
    InternalError(String),
}

impl ServerError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::RoomNotFound => StatusCode::NOT_FOUND,
            Self::Unauthorized | Self::Banned => StatusCode::FORBIDDEN,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::RoomFull | Self::MaxRoomsReached => StatusCode::CONFLICT,
            _ => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::RoomNotFound => "Room not found".into(),
            Self::RoomFull => "Room is full".into(),
            Self::RoomCodeInvalid => "Invalid room code format".into(),
            Self::MaxRoomsReached => "Maximum number of rooms reached".into(),
            Self::InvalidSlot(s) => format!("Invalid slot: {s}"),
            Self::SlotEmpty => "Slot is already empty".into(),
            Self::SlotOccupied => "Slot is occupied".into(),
            Self::CannotModifyCreator => "Cannot modify the room creator".into(),
            Self::CannotBanSameIp => "Cannot ban this player — they share your IP address".into(),
            Self::NotInLobby => "Cannot perform this action during game".into(),
            Self::NotInGame => "No game in progress".into(),
            Self::NotYourTurn => "Not your turn".into(),
            Self::GameNotStarted => "Game has not started".into(),
            Self::GameAlreadyStarted => "Game has already started".into(),
            Self::NotAllSlotsFilled => "Not all player slots are filled".into(),
            Self::InvalidAction(msg) => format!("Invalid action: {msg}"),
            Self::InvalidPosition(p) => format!("Invalid position: {p}"),
            Self::NotHost => "Only the host can perform this action".into(),
            Self::Unauthorized => "Unauthorized".into(),
            Self::Banned => "You are banned from this room".into(),
            Self::RateLimited => "Too many requests, please slow down".into(),
            Self::PlayerNameTooLong => "Player name must be 32 characters or fewer".into(),
            Self::PlayerNameEmpty => "Player name cannot be empty".into(),
            Self::InvalidTurnTimer => "Turn timer must be between 10 and 300 seconds".into(),
            Self::InvalidDisconnectTimeout => "Disconnect timeout must be between 10 and 300 seconds".into(),
            Self::InvalidNumPlayers => "Number of players must be between 2 and 8".into(),
            Self::InvalidStrategy(s) => format!("Unknown strategy: {s}"),
            Self::InvalidRules(r) => format!("Unknown rules: {r}"),
            Self::InternalError(msg) => format!("Internal error: {msg}"),
        }
    }
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ServerError {}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = serde_json::json!({
            "error": {
                "code": format!("{:?}", self),
                "message": self.message(),
            }
        });
        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_to_http_status_codes() {
        assert_eq!(ServerError::RoomNotFound.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(ServerError::Unauthorized.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(ServerError::Banned.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(ServerError::RateLimited.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(ServerError::InternalError("test".into()).status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(ServerError::RoomFull.status_code(), StatusCode::CONFLICT);
        assert_eq!(ServerError::InvalidSlot(5).status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(ServerError::NotYourTurn.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn error_messages_not_empty() {
        let errors = vec![
            ServerError::RoomNotFound,
            ServerError::RoomFull,
            ServerError::InvalidSlot(0),
            ServerError::NotYourTurn,
            ServerError::Unauthorized,
            ServerError::RateLimited,
            ServerError::PlayerNameTooLong,
            ServerError::InvalidAction("test".into()),
        ];
        for err in errors {
            assert!(!err.message().is_empty(), "Empty message for {:?}", err);
        }
    }

    #[test]
    fn error_display_matches_message() {
        let err = ServerError::RoomNotFound;
        assert_eq!(format!("{err}"), err.message());
    }
}
