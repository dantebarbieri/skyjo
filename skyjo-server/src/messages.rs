use serde::{Deserialize, Serialize};
use skyjo_core::interactive::{InteractiveGameState, PlayerAction};

/// Messages sent from the client to the server over WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ClientMessage {
    /// Configure a player slot in the lobby (creator only).
    ConfigureSlot { slot: usize, player_type: String },
    /// Change the number of player slots (creator only, lobby phase).
    SetNumPlayers { num_players: usize },
    /// Change the rule set (creator only, lobby phase).
    SetRules { rules: String },
    /// Kick a player from the room (creator only).
    KickPlayer { slot: usize },
    /// Ban a player from the room by IP (creator only). IP is never exposed.
    BanPlayer { slot: usize },
    /// Promote a player to host (creator only).
    PromoteHost { slot: usize },
    /// Return to lobby after game ends (preserves room and players).
    ReturnToLobby,
    /// Start the game (creator only).
    StartGame,
    /// Submit a game action. The server derives the player from the session.
    Action { action: PlayerAction },
    /// Continue to next round (any player can trigger).
    ContinueRound,
    /// Start a new game after game over (creator only).
    PlayAgain,
    /// Set the turn timer (creator only, lobby phase).
    SetTurnTimer {
        /// Seconds per turn, or null for unlimited.
        secs: Option<u64>,
    },
    /// Set the disconnect-to-bot timeout (creator only, lobby phase).
    SetDisconnectTimeout {
        /// Seconds before a disconnected player becomes a bot, or null for default (60s).
        secs: Option<u32>,
    },
    /// Keepalive ping.
    Ping,
}

/// Messages sent from the server to the client over WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Current lobby state (sent on join and on changes).
    RoomState { state: RoomLobbyState },
    /// Full game state update for this player's perspective.
    GameState {
        state: InteractiveGameState,
        /// Seconds remaining for the current player's turn (None if unlimited or not their turn).
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
    },
    /// A player's action was applied (includes who and what for animation).
    ActionApplied {
        player: usize,
        action: PlayerAction,
        state: InteractiveGameState,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
    },
    /// A bot action was applied.
    BotAction {
        player: usize,
        action: PlayerAction,
        state: InteractiveGameState,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
    },
    /// A timeout-triggered random action was applied.
    TimeoutAction {
        player: usize,
        action: PlayerAction,
        state: InteractiveGameState,
    },
    /// A player joined the room.
    PlayerJoined { player_index: usize, name: String },
    /// A player disconnected.
    PlayerLeft { player_index: usize },
    /// A player reconnected.
    PlayerReconnected { player_index: usize },
    /// A disconnected player was converted to a bot.
    PlayerConvertedToBot { slot: usize, name: String },
    /// You were kicked from the room.
    Kicked { reason: String },
    /// An error in response to a client message.
    Error { code: String, message: String },
    /// Keepalive pong.
    Pong,
}

/// Lobby state broadcast to all connected players.
#[derive(Debug, Clone, Serialize)]
pub struct RoomLobbyState {
    pub room_code: String,
    pub players: Vec<LobbyPlayer>,
    pub num_players: usize,
    pub rules: String,
    pub creator: usize,
    pub available_strategies: Vec<String>,
    pub available_rules: Vec<String>,
    /// Seconds remaining before the room is auto-deleted (None if no timeout applies).
    pub idle_timeout_secs: Option<u64>,
    /// Turn timer setting: seconds per turn, or None for unlimited.
    pub turn_timer_secs: Option<u64>,
    /// Disconnect-to-bot timeout: seconds before a disconnected player becomes a bot, or None for default (60s).
    pub disconnect_bot_timeout_secs: Option<u32>,
    /// Player indices who won the last game (shown as crowns in lobby).
    pub last_winners: Vec<usize>,
    /// Number of games the genetic bot model has been trained on.
    pub genetic_games_trained: usize,
    /// Current generation of the genetic bot model.
    pub genetic_generation: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LobbyPlayer {
    pub slot: usize,
    pub name: String,
    pub player_type: PlayerSlotType,
    pub connected: bool,
    /// True if this player shares an IP with the room creator (shown only to creator).
    /// Used to warn before banning. Never reveals the actual IP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares_ip_with_host: Option<bool>,
    /// Seconds since this player disconnected (None if connected or non-human).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disconnect_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum PlayerSlotType {
    Human,
    Bot { strategy: String },
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- PlayerSlotType serde round-trips ---

    #[test]
    fn player_slot_type_human_round_trip() {
        let slot = PlayerSlotType::Human;
        let json = serde_json::to_string(&slot).unwrap();
        let parsed: PlayerSlotType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PlayerSlotType::Human);
    }

    #[test]
    fn player_slot_type_bot_round_trip() {
        let slot = PlayerSlotType::Bot {
            strategy: "SmartBot".to_string(),
        };
        let json = serde_json::to_string(&slot).unwrap();
        let parsed: PlayerSlotType = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            PlayerSlotType::Bot {
                strategy: "SmartBot".to_string()
            }
        );
    }

    #[test]
    fn player_slot_type_empty_round_trip() {
        let slot = PlayerSlotType::Empty;
        let json = serde_json::to_string(&slot).unwrap();
        let parsed: PlayerSlotType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PlayerSlotType::Empty);
    }

    #[test]
    fn player_slot_type_serializes_with_kind_tag() {
        let human: serde_json::Value = serde_json::to_value(PlayerSlotType::Human).unwrap();
        assert_eq!(human["kind"], "Human");

        let bot: serde_json::Value = serde_json::to_value(PlayerSlotType::Bot {
            strategy: "X".to_string(),
        })
        .unwrap();
        assert_eq!(bot["kind"], "Bot");
        assert_eq!(bot["strategy"], "X");

        let empty: serde_json::Value = serde_json::to_value(PlayerSlotType::Empty).unwrap();
        assert_eq!(empty["kind"], "Empty");
    }

    // --- ClientMessage deserialization ---

    #[test]
    fn client_message_configure_slot() {
        let json = r#"{"type":"ConfigureSlot","slot":1,"player_type":"Human"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::ConfigureSlot { slot: 1, .. }));
    }

    #[test]
    fn client_message_set_num_players() {
        let json = r#"{"type":"SetNumPlayers","num_players":4}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ClientMessage::SetNumPlayers { num_players: 4 }
        ));
    }

    #[test]
    fn client_message_set_rules() {
        let json = r#"{"type":"SetRules","rules":"Standard"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::SetRules { .. }));
    }

    #[test]
    fn client_message_kick_player() {
        let json = r#"{"type":"KickPlayer","slot":2}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::KickPlayer { slot: 2 }));
    }

    #[test]
    fn client_message_ban_player() {
        let json = r#"{"type":"BanPlayer","slot":3}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::BanPlayer { slot: 3 }));
    }

    #[test]
    fn client_message_promote_host() {
        let json = r#"{"type":"PromoteHost","slot":0}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::PromoteHost { slot: 0 }));
    }

    #[test]
    fn client_message_return_to_lobby() {
        let json = r#"{"type":"ReturnToLobby"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::ReturnToLobby));
    }

    #[test]
    fn client_message_start_game() {
        let json = r#"{"type":"StartGame"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::StartGame));
    }

    #[test]
    fn client_message_action() {
        let json = r#"{"type":"Action","action":{"type":"DrawFromDeck"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Action { .. }));
    }

    #[test]
    fn client_message_continue_round() {
        let json = r#"{"type":"ContinueRound"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::ContinueRound));
    }

    #[test]
    fn client_message_play_again() {
        let json = r#"{"type":"PlayAgain"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::PlayAgain));
    }

    #[test]
    fn client_message_set_turn_timer_with_value() {
        let json = r#"{"type":"SetTurnTimer","secs":30}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ClientMessage::SetTurnTimer { secs: Some(30) }
        ));
    }

    #[test]
    fn client_message_set_turn_timer_null() {
        let json = r#"{"type":"SetTurnTimer","secs":null}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::SetTurnTimer { secs: None }));
    }

    #[test]
    fn client_message_ping() {
        let json = r#"{"type":"Ping"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn client_message_set_disconnect_timeout_with_value() {
        let json = r#"{"type":"SetDisconnectTimeout","secs":120}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ClientMessage::SetDisconnectTimeout { secs: Some(120) }
        ));
    }

    #[test]
    fn client_message_set_disconnect_timeout_null() {
        let json = r#"{"type":"SetDisconnectTimeout","secs":null}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ClientMessage::SetDisconnectTimeout { secs: None }
        ));
    }

    #[test]
    fn client_message_rejects_unknown_type() {
        let json = r#"{"type":"Unknown"}"#;
        assert!(serde_json::from_str::<ClientMessage>(json).is_err());
    }

    #[test]
    fn client_message_rejects_missing_required_fields() {
        let json = r#"{"type":"ConfigureSlot","slot":1}"#;
        assert!(serde_json::from_str::<ClientMessage>(json).is_err());
    }

    // --- ServerMessage serialization ---

    #[test]
    fn server_message_error_serializes() {
        let msg = ServerMessage::Error {
            code: "NOT_FOUND".to_string(),
            message: "Room not found".to_string(),
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "Error");
        assert_eq!(val["code"], "NOT_FOUND");
        assert_eq!(val["message"], "Room not found");
    }

    #[test]
    fn server_message_pong_serializes() {
        let msg = ServerMessage::Pong;
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "Pong");
    }

    #[test]
    fn server_message_kicked_serializes() {
        let msg = ServerMessage::Kicked {
            reason: "Bad behavior".to_string(),
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "Kicked");
        assert_eq!(val["reason"], "Bad behavior");
    }

    #[test]
    fn server_message_player_joined_serializes() {
        let msg = ServerMessage::PlayerJoined {
            player_index: 2,
            name: "Alice".to_string(),
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "PlayerJoined");
        assert_eq!(val["player_index"], 2);
        assert_eq!(val["name"], "Alice");
    }

    #[test]
    fn server_message_player_converted_to_bot_serializes() {
        let msg = ServerMessage::PlayerConvertedToBot {
            slot: 1,
            name: "Alice (Bot)".to_string(),
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "PlayerConvertedToBot");
        assert_eq!(val["slot"], 1);
        assert_eq!(val["name"], "Alice (Bot)");
    }

    #[test]
    fn server_message_player_left_serializes() {
        let msg = ServerMessage::PlayerLeft { player_index: 1 };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "PlayerLeft");
        assert_eq!(val["player_index"], 1);
    }

    #[test]
    fn server_message_player_reconnected_serializes() {
        let msg = ServerMessage::PlayerReconnected { player_index: 0 };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "PlayerReconnected");
        assert_eq!(val["player_index"], 0);
    }

    #[test]
    fn server_message_room_state_serializes() {
        let state = RoomLobbyState {
            room_code: "ABCD".to_string(),
            players: vec![],
            num_players: 2,
            rules: "Standard".to_string(),
            creator: 0,
            available_strategies: vec!["Random".to_string()],
            available_rules: vec!["Standard".to_string()],
            idle_timeout_secs: Some(300),
            turn_timer_secs: None,
            disconnect_bot_timeout_secs: None,
            last_winners: vec![],
            genetic_games_trained: 0,
            genetic_generation: 0,
        };
        let msg = ServerMessage::RoomState { state };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "RoomState");
        assert_eq!(val["state"]["room_code"], "ABCD");
        assert_eq!(val["state"]["num_players"], 2);
    }

    #[test]
    fn lobby_player_skips_none_optional_fields() {
        let player = LobbyPlayer {
            slot: 0,
            name: "Bob".to_string(),
            player_type: PlayerSlotType::Human,
            connected: true,
            shares_ip_with_host: None,
            disconnect_secs: None,
        };
        let val: serde_json::Value = serde_json::to_value(&player).unwrap();
        assert!(val.get("shares_ip_with_host").is_none());
        assert!(val.get("disconnect_secs").is_none());
    }

    #[test]
    fn lobby_player_includes_some_optional_fields() {
        let player = LobbyPlayer {
            slot: 1,
            name: "Eve".to_string(),
            player_type: PlayerSlotType::Bot {
                strategy: "Smart".to_string(),
            },
            connected: false,
            shares_ip_with_host: Some(true),
            disconnect_secs: Some(45),
        };
        let val: serde_json::Value = serde_json::to_value(&player).unwrap();
        assert_eq!(val["shares_ip_with_host"], true);
        assert_eq!(val["disconnect_secs"], 45);
    }
}
