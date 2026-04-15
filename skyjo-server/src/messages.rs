use serde::{Deserialize, Serialize};
use skyjo_core::VisibleSlot;
use skyjo_core::interactive::{ActionNeeded, InteractiveGameState, PlayerAction};

/// Wire format for WebSocket messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFormat {
    Json,
    MessagePack,
}

/// Messages sent from the client to the server over WebSocket.
#[derive(Debug, Serialize, Deserialize)]
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
    /// Signal readiness for the next round (per-player).
    ReadyForNextRound,
    /// Set lobby ready state (any human player).
    SetReady { ready: bool },
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
    /// Request a full game state resync.
    RequestFullState,
}

/// Messages sent from the server to the client over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        /// Per-player ready state for round continuation (present only during RoundOver).
        #[serde(skip_serializing_if = "Option::is_none")]
        round_ready: Option<Vec<bool>>,
    },
    /// A player's action was applied (includes who and what for animation).
    ActionApplied {
        player: usize,
        action: PlayerAction,
        state: InteractiveGameState,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        round_ready: Option<Vec<bool>>,
    },
    /// A bot action was applied.
    BotAction {
        player: usize,
        action: PlayerAction,
        state: InteractiveGameState,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        round_ready: Option<Vec<bool>>,
    },
    /// A timeout-triggered random action was applied.
    TimeoutAction {
        player: usize,
        action: PlayerAction,
        state: InteractiveGameState,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        round_ready: Option<Vec<bool>>,
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
    /// A delta update for a player action (compact alternative to ActionApplied).
    ActionAppliedDelta {
        player: usize,
        action: PlayerAction,
        delta: StateDelta,
    },
    /// Server is shutting down. Clients should reconnect after a brief delay.
    ServerShutdown,
}

impl ServerMessage {
    /// Serialize to the specified wire format.
    pub fn to_bytes(&self, format: WireFormat) -> Vec<u8> {
        match format {
            WireFormat::Json => {
                serde_json::to_vec(self).expect("ServerMessage serialization failed")
            }
            WireFormat::MessagePack => {
                rmp_serde::to_vec_named(self).expect("ServerMessage msgpack serialization failed")
            }
        }
    }
}

impl ClientMessage {
    /// Deserialize from bytes, auto-detecting format.
    /// Binary data → MessagePack, text data → JSON.
    pub fn from_bytes(data: &[u8], is_binary: bool) -> Result<Self, String> {
        if is_binary {
            rmp_serde::from_slice(data).map_err(|e| format!("MessagePack decode error: {e}"))
        } else {
            serde_json::from_slice(data).map_err(|e| format!("JSON decode error: {e}"))
        }
    }
}

/// Lobby state broadcast to all connected players.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub slot: usize,
    pub name: String,
    pub player_type: PlayerSlotType,
    pub connected: bool,
    /// Whether this player is ready (host and bots are always ready).
    pub ready: bool,
    /// True if this player shares an IP with the room creator (shown only to creator).
    /// Used to warn before banning. Never reveals the actual IP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares_ip_with_host: Option<bool>,
    /// Seconds since this player disconnected (None if connected or non-human).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disconnect_secs: Option<u64>,
    /// Last measured ping round-trip time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
    /// Number of broadcast lag events (channel overflow).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub broadcast_lag_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum PlayerSlotType {
    Human,
    Bot { strategy: String },
    Empty,
}

/// Compact slot update for delta messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SlotUpdate {
    Hidden,
    Revealed(i8),
    Cleared,
}

/// Compact representation of changes from a single action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    /// Board slot changes: (player_index, slot_position, new_visible_slot_value)
    pub board_changes: Vec<(usize, usize, SlotUpdate)>,
    /// Changed discard pile tops: (pile_index, new_top_value_or_none)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub discard_tops_changed: Vec<(usize, Option<i8>)>,
    /// Updated deck remaining count
    pub deck_remaining: usize,
    /// Who plays next
    pub current_player: usize,
    /// Columns cleared: (player_index, column_index)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub column_clears: Vec<(usize, usize)>,
    /// What the current player needs to do next
    pub action_needed: String,
    /// Turn deadline (if applicable)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub turn_deadline_secs: Option<f64>,
    /// Is this the final turn (someone went out)?
    pub is_final_turn: bool,
    /// Going out player index (if changed)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub going_out_player: Option<usize>,
}

/// Convert a `VisibleSlot` to a `SlotUpdate`.
pub fn slot_to_update(slot: &VisibleSlot) -> SlotUpdate {
    match slot {
        VisibleSlot::Hidden => SlotUpdate::Hidden,
        VisibleSlot::Revealed(v) => SlotUpdate::Revealed(*v),
        VisibleSlot::Cleared => SlotUpdate::Cleared,
    }
}

/// Compute the delta between before and after game states.
pub fn compute_delta(before: &InteractiveGameState, after: &InteractiveGameState) -> StateDelta {
    let mut board_changes = Vec::new();

    // Compare boards
    for (player_idx, (before_board, after_board)) in
        before.boards.iter().zip(after.boards.iter()).enumerate()
    {
        for (slot_idx, (before_slot, after_slot)) in
            before_board.iter().zip(after_board.iter()).enumerate()
        {
            if before_slot != after_slot {
                board_changes.push((player_idx, slot_idx, slot_to_update(after_slot)));
            }
        }
    }

    // Compare discard tops (support multiple piles)
    let mut discard_tops_changed = Vec::new();
    for (i, (before_top, after_top)) in before
        .discard_tops
        .iter()
        .zip(after.discard_tops.iter())
        .enumerate()
    {
        if before_top != after_top {
            discard_tops_changed.push((i, *after_top));
        }
    }
    // Handle case where after has more piles
    for (i, top) in after
        .discard_tops
        .iter()
        .enumerate()
        .skip(before.discard_tops.len())
    {
        discard_tops_changed.push((i, *top));
    }

    // Column clears
    let column_clears: Vec<(usize, usize)> = after
        .last_column_clears
        .iter()
        .map(|e| (e.player_index, e.column))
        .collect();

    StateDelta {
        board_changes,
        discard_tops_changed,
        deck_remaining: after.deck_remaining,
        current_player: after.current_player,
        column_clears,
        action_needed: action_needed_name(&after.action_needed),
        turn_deadline_secs: None, // Set by caller
        is_final_turn: after.is_final_turn,
        going_out_player: if before.going_out_player != after.going_out_player {
            after.going_out_player
        } else {
            None
        },
    }
}

/// Return a short phase name for an `ActionNeeded` variant.
fn action_needed_name(action: &ActionNeeded) -> String {
    match action {
        ActionNeeded::ChooseInitialFlips { .. } => "ChooseInitialFlips".to_string(),
        ActionNeeded::ChooseDraw { .. } => "ChooseDraw".to_string(),
        ActionNeeded::ChooseDeckDrawAction { .. } => "ChooseDeckDrawAction".to_string(),
        ActionNeeded::ChooseDiscardDrawPlacement { .. } => "ChooseDiscardDrawPlacement".to_string(),
        ActionNeeded::RoundOver { .. } => "RoundOver".to_string(),
        ActionNeeded::GameOver { .. } => "GameOver".to_string(),
    }
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
            ready: true,
            shares_ip_with_host: None,
            disconnect_secs: None,
            latency_ms: None,
            broadcast_lag_count: 0,
        };
        let val: serde_json::Value = serde_json::to_value(&player).unwrap();
        assert!(val.get("shares_ip_with_host").is_none());
        assert!(val.get("disconnect_secs").is_none());
        assert!(val.get("latency_ms").is_none());
        assert!(val.get("broadcast_lag_count").is_none());
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
            ready: true,
            shares_ip_with_host: Some(true),
            disconnect_secs: Some(45),
            latency_ms: Some(42),
            broadcast_lag_count: 3,
        };
        let val: serde_json::Value = serde_json::to_value(&player).unwrap();
        assert_eq!(val["shares_ip_with_host"], true);
        assert_eq!(val["disconnect_secs"], 45);
    }

    // --- MessagePack round-trip tests ---

    #[test]
    fn msgpack_round_trip_server_message() {
        let msg = ServerMessage::Error {
            code: "test".to_string(),
            message: "Test error".to_string(),
        };
        let bytes = rmp_serde::to_vec_named(&msg).unwrap();
        let decoded: ServerMessage = rmp_serde::from_slice(&bytes).unwrap();
        match decoded {
            ServerMessage::Error { code, message } => {
                assert_eq!(code, "test");
                assert_eq!(message, "Test error");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn msgpack_round_trip_client_message() {
        let msg = ClientMessage::Ping;
        let bytes = rmp_serde::to_vec_named(&msg).unwrap();
        let decoded: ClientMessage = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(decoded, ClientMessage::Ping));
    }

    #[test]
    fn msgpack_round_trip_game_state() {
        let msg = ServerMessage::Pong;
        let bytes = rmp_serde::to_vec_named(&msg).unwrap();
        let decoded: ServerMessage = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(decoded, ServerMessage::Pong));
    }

    #[test]
    fn json_client_message_from_bytes() {
        let json = br#"{"type":"Ping"}"#;
        let msg = ClientMessage::from_bytes(json, false).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn msgpack_client_message_from_bytes() {
        let msg = ClientMessage::Ping;
        let bytes = rmp_serde::to_vec_named(&msg).unwrap();
        let decoded = ClientMessage::from_bytes(&bytes, true).unwrap();
        assert!(matches!(decoded, ClientMessage::Ping));
    }

    #[test]
    fn msgpack_smaller_than_json() {
        let msg = ServerMessage::Error {
            code: "RoomNotFound".to_string(),
            message: "Room not found".to_string(),
        };
        let json_bytes = serde_json::to_vec(&msg).unwrap();
        let msgpack_bytes = rmp_serde::to_vec_named(&msg).unwrap();
        assert!(
            msgpack_bytes.len() < json_bytes.len(),
            "MessagePack ({}) should be smaller than JSON ({})",
            msgpack_bytes.len(),
            json_bytes.len()
        );
    }

    #[test]
    fn server_message_to_bytes_json() {
        let msg = ServerMessage::Pong;
        let bytes = msg.to_bytes(WireFormat::Json);
        let decoded: ServerMessage = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(decoded, ServerMessage::Pong));
    }

    #[test]
    fn server_message_to_bytes_msgpack() {
        let msg = ServerMessage::Pong;
        let bytes = msg.to_bytes(WireFormat::MessagePack);
        let decoded: ServerMessage = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(decoded, ServerMessage::Pong));
    }

    // --- Delta state update tests ---

    fn make_test_state(
        boards: Vec<Vec<VisibleSlot>>,
        discard_top: Option<i8>,
    ) -> InteractiveGameState {
        use skyjo_core::interactive::ActionNeeded;
        InteractiveGameState {
            num_players: boards.len(),
            player_names: (0..boards.len()).map(|i| format!("P{i}")).collect(),
            num_rows: 3,
            num_cols: 4,
            round_number: 1,
            current_player: 0,
            action_needed: ActionNeeded::ChooseDraw {
                player: 0,
                drawable_piles: vec![0],
            },
            boards,
            discard_tops: vec![discard_top],
            discard_sizes: vec![1],
            deck_remaining: 100,
            cumulative_scores: vec![0, 0],
            going_out_player: None,
            is_final_turn: false,
            last_column_clears: vec![],
        }
    }

    #[test]
    fn delta_detects_board_changes() {
        let before = make_test_state(
            vec![vec![VisibleSlot::Hidden; 12], vec![VisibleSlot::Hidden; 12]],
            Some(5),
        );
        let mut after = before.clone();
        after.boards[0][3] = VisibleSlot::Revealed(7);

        let delta = compute_delta(&before, &after);
        assert_eq!(delta.board_changes.len(), 1);
        assert_eq!(delta.board_changes[0], (0, 3, SlotUpdate::Revealed(7)));
    }

    #[test]
    fn delta_empty_when_no_changes() {
        let state = make_test_state(
            vec![vec![VisibleSlot::Hidden; 12], vec![VisibleSlot::Hidden; 12]],
            Some(5),
        );
        let delta = compute_delta(&state, &state);
        assert!(delta.board_changes.is_empty());
        assert!(delta.discard_tops_changed.is_empty());
        assert!(delta.column_clears.is_empty());
        assert!(delta.going_out_player.is_none());
    }

    #[test]
    fn msgpack_round_trip_delta() {
        let delta = StateDelta {
            board_changes: vec![(0, 3, SlotUpdate::Revealed(7))],
            discard_tops_changed: vec![(0, Some(5))],
            deck_remaining: 99,
            current_player: 1,
            column_clears: vec![],
            action_needed: "ChooseDraw".to_string(),
            turn_deadline_secs: None,
            is_final_turn: false,
            going_out_player: None,
        };
        // Use named serialization for reliable round-trip with skip_serializing_if
        let bytes = rmp_serde::to_vec_named(&delta).unwrap();
        let decoded: StateDelta = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.board_changes.len(), 1);
        assert_eq!(decoded.board_changes[0], (0, 3, SlotUpdate::Revealed(7)));
        assert_eq!(decoded.discard_tops_changed, vec![(0, Some(5))]);
        assert_eq!(decoded.deck_remaining, 99);
        assert_eq!(decoded.current_player, 1);
    }

    #[test]
    fn request_full_state_message_parses() {
        let msg: ClientMessage = serde_json::from_str(r#"{"type":"RequestFullState"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::RequestFullState));
    }

    #[test]
    fn action_applied_delta_serializes() {
        let delta = StateDelta {
            board_changes: vec![(1, 5, SlotUpdate::Cleared)],
            discard_tops_changed: vec![],
            deck_remaining: 80,
            current_player: 0,
            column_clears: vec![(1, 2)],
            action_needed: "ChooseDraw".to_string(),
            turn_deadline_secs: Some(25.5),
            is_final_turn: true,
            going_out_player: Some(1),
        };
        let msg = ServerMessage::ActionAppliedDelta {
            player: 1,
            action: PlayerAction::DrawFromDeck,
            delta,
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "ActionAppliedDelta");
        assert_eq!(val["player"], 1);
        assert_eq!(val["delta"]["deck_remaining"], 80);
        assert_eq!(val["delta"]["is_final_turn"], true);
    }

    #[test]
    fn slot_update_serde_round_trip() {
        let updates = vec![
            SlotUpdate::Hidden,
            SlotUpdate::Revealed(-2),
            SlotUpdate::Cleared,
        ];
        let json = serde_json::to_string(&updates).unwrap();
        let decoded: Vec<SlotUpdate> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, updates);
    }

    #[test]
    fn delta_detects_discard_change() {
        let before = make_test_state(vec![vec![VisibleSlot::Hidden; 12]; 2], Some(3));
        let mut after = before.clone();
        after.discard_tops = vec![Some(9)];

        let delta = compute_delta(&before, &after);
        assert_eq!(delta.discard_tops_changed, vec![(0, Some(9))]);
    }

    #[test]
    fn delta_detects_going_out_player_change() {
        let before = make_test_state(vec![vec![VisibleSlot::Hidden; 12]; 2], Some(3));
        let mut after = before.clone();
        after.going_out_player = Some(0);
        after.is_final_turn = true;

        let delta = compute_delta(&before, &after);
        assert_eq!(delta.going_out_player, Some(0));
        assert!(delta.is_final_turn);
    }

    #[test]
    fn server_shutdown_message_serializes() {
        let msg = ServerMessage::ServerShutdown;
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "ServerShutdown");
    }

    #[test]
    fn server_shutdown_message_round_trip_json() {
        let msg = ServerMessage::ServerShutdown;
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ServerMessage::ServerShutdown));
    }

    #[test]
    fn server_shutdown_message_round_trip_msgpack() {
        let msg = ServerMessage::ServerShutdown;
        let bytes = rmp_serde::to_vec_named(&msg).unwrap();
        let decoded: ServerMessage = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(decoded, ServerMessage::ServerShutdown));
    }

    // --- TimeoutAction turn_deadline_secs tests ---

    #[test]
    fn timeout_action_serialization_with_deadline() {
        let state = make_test_state(vec![vec![VisibleSlot::Hidden; 12]; 2], Some(5));
        let msg = ServerMessage::TimeoutAction {
            player: 0,
            action: PlayerAction::DrawFromDeck,
            state: state.clone(),
            turn_deadline_secs: Some(30),
            round_ready: None,
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "TimeoutAction");
        assert_eq!(val["player"], 0);
        assert_eq!(val["turn_deadline_secs"], 30);

        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            ServerMessage::TimeoutAction {
                player,
                turn_deadline_secs,
                ..
            } => {
                assert_eq!(player, 0);
                assert_eq!(turn_deadline_secs, Some(30));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn timeout_action_without_deadline_skips_field() {
        let state = make_test_state(vec![vec![VisibleSlot::Hidden; 12]; 2], Some(5));
        let msg = ServerMessage::TimeoutAction {
            player: 1,
            action: PlayerAction::DrawFromDeck,
            state,
            turn_deadline_secs: None,
            round_ready: None,
        };
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(val["type"], "TimeoutAction");
        assert!(
            val.get("turn_deadline_secs").is_none(),
            "None should be skipped via skip_serializing_if"
        );
    }
}
