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
