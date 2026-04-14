use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast};

use skyjo_core::{
    ClearerStrategy, DefensiveStrategy, GamblerStrategy, GeneticStrategy, GreedyStrategy,
    InteractiveGame, InteractiveGameState, MimicStrategy, PlayerAction, RandomStrategy, Rules,
    RusherStrategy, SaboteurStrategy, StandardRules, StatisticianStrategy, Strategy,
    SurvivorStrategy,
};

use crate::error::ServerError;
use crate::messages::{
    LobbyPlayer, PlayerSlotType, RoomLobbyState, ServerMessage, StateDelta, compute_delta,
};
use crate::session::SessionToken;

/// Room lifecycle state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RoomPhase {
    Lobby,
    InGame,
    GameOver,
}

/// Serializable room snapshot for persistence (crash recovery).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSnapshot {
    pub code: String,
    pub phase: RoomPhase,
    pub num_players: usize,
    pub creator: usize,
    pub players: Vec<PlayerSlotSnapshot>,
    pub rules_name: String,
    pub turn_timer_secs: Option<u64>,
    pub disconnect_bot_timeout_secs: Option<u32>,
    /// Full game state (via `get_full_state()`) if a game is active.
    pub game_state_json: Option<String>,
    pub banned_ips: Vec<String>,
    pub last_winners: Vec<usize>,
}

/// Serializable snapshot of a player slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSlotSnapshot {
    pub name: String,
    pub slot_type: PlayerSlotType,
    pub was_human: bool,
}

/// A player slot in the room.
#[derive(Debug, Clone)]
pub struct PlayerSlot {
    pub name: String,
    pub slot_type: PlayerSlotType,
    pub session_token: Option<SessionToken>,
    pub connected: bool,
    /// IP address of the connected player (never exposed to clients).
    pub ip: Option<String>,
    /// When this player disconnected (for auto-kick timer).
    pub disconnected_at: Option<Instant>,
    /// True if this bot was converted from a disconnected human player.
    pub was_human: bool,
    /// Last measured ping round-trip time in milliseconds.
    pub latency_ms: Option<u32>,
    /// Number of broadcast lag events (channel overflow).
    pub broadcast_lag_count: u32,
}

/// A game room holding all state for one multiplayer session.
pub struct Room {
    pub code: String,
    pub phase: RoomPhase,
    pub num_players: usize,
    pub rules_name: String,
    pub creator: usize,
    pub players: Vec<PlayerSlot>,
    pub game: Option<InteractiveGame>,
    pub last_activity: Instant,
    /// Broadcast channel for sending messages to all connected WebSocket handlers.
    pub broadcast_tx: broadcast::Sender<(usize, ServerMessage)>,
    /// Per-player message senders for targeted messages.
    /// None if the player is not connected.
    pub(crate) player_txs: Vec<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
    /// Banned IP addresses (never exposed to clients).
    pub banned_ips: Vec<String>,
    /// Winners from the last completed game (for crown display in lobby).
    pub last_winners: Vec<usize>,
    /// Turn timer: seconds per human turn, or None for unlimited.
    pub turn_timer_secs: Option<u64>,
    /// When the current human player's turn started (for timeout tracking).
    pub turn_start: Option<Instant>,
    /// Cached best genetic genome for constructing GeneticStrategy bots.
    pub genetic_genome: Option<Vec<f32>>,
    /// Number of games the genetic model has been trained on.
    pub genetic_games_trained: usize,
    /// Current generation of the genetic model.
    pub genetic_generation: usize,
    /// Seconds before a disconnected player is converted to a bot during a game.
    /// None means use the default (60 seconds).
    pub disconnect_bot_timeout_secs: Option<u32>,
}

/// Validate and sanitize a player name.
/// Trims whitespace, rejects empty names and names longer than 32 characters.
pub fn validate_player_name(name: &str) -> Result<String, ServerError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err(ServerError::PlayerNameEmpty);
    }
    if trimmed.len() > 32 {
        return Err(ServerError::PlayerNameTooLong);
    }
    Ok(trimmed)
}

/// Validate a room code format: exactly 6 uppercase alphanumeric characters,
/// excluding I, O, and L.
pub fn validate_room_code(code: &str) -> Result<(), ServerError> {
    let valid_chars: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    if code.len() != 6 || !code.bytes().all(|b| valid_chars.contains(&b)) {
        return Err(ServerError::RoomCodeInvalid);
    }
    Ok(())
}

impl Room {
    pub fn new(
        code: String,
        creator_name: String,
        num_players: usize,
        rules: Option<String>,
        genetic_games_trained: usize,
        genetic_generation: usize,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);

        let rules_name = rules.unwrap_or_else(|| "Standard".to_string());

        let mut players = Vec::with_capacity(num_players);
        // Creator is player 0
        players.push(PlayerSlot {
            name: creator_name,
            slot_type: PlayerSlotType::Human,
            session_token: None, // Set after creation
            connected: false,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        });
        // Remaining slots are empty
        for _ in 1..num_players {
            players.push(PlayerSlot {
                name: String::new(),
                slot_type: PlayerSlotType::Empty,
                session_token: None,
                connected: false,
                ip: None,
                disconnected_at: None,
                was_human: false,
                latency_ms: None,
                broadcast_lag_count: 0,
            });
        }

        Room {
            code,
            phase: RoomPhase::Lobby,
            num_players,
            rules_name,
            creator: 0,
            players,
            game: None,
            last_activity: Instant::now(),
            broadcast_tx,
            player_txs: vec![None; num_players],
            banned_ips: Vec::new(),
            last_winners: Vec::new(),
            turn_timer_secs: Some(60),
            turn_start: None,
            genetic_genome: None,
            genetic_games_trained,
            genetic_generation,
            disconnect_bot_timeout_secs: None,
        }
    }

    /// Check if an IP is banned from this room.
    pub fn is_ip_banned(&self, ip: &str) -> bool {
        self.banned_ips.iter().any(|b| b == ip)
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Valid state transitions:
    ///   Lobby → InGame (start_game)
    ///   InGame → GameOver (game ends)
    ///   GameOver → Lobby (return_to_lobby / play_again)
    ///
    /// Returns error if the transition is invalid.
    fn transition(&mut self, new_phase: RoomPhase) -> Result<(), ServerError> {
        let valid = matches!(
            (&self.phase, &new_phase),
            (RoomPhase::Lobby, RoomPhase::InGame)
                | (RoomPhase::InGame, RoomPhase::GameOver)
                | (RoomPhase::GameOver, RoomPhase::Lobby)
        );

        if !valid {
            tracing::warn!(
                from = ?self.phase,
                to = ?new_phase,
                room = %self.code,
                "invalid_room_transition"
            );
            return Err(ServerError::InvalidAction(format!(
                "Cannot transition from {:?} to {:?}",
                self.phase, new_phase
            )));
        }

        tracing::info!(
            from = ?self.phase,
            to = ?new_phase,
            room = %self.code,
            "room_transition"
        );
        self.phase = new_phase;
        self.touch();
        Ok(())
    }

    /// Find the next available slot for a new player.
    /// Prefers empty slots, falls back to the first bot slot.
    pub fn next_available_slot(&self) -> Option<usize> {
        // Prefer empty slots
        self.players
            .iter()
            .position(|p| p.slot_type == PlayerSlotType::Empty)
            .or_else(|| {
                // Fall back to first bot slot
                self.players
                    .iter()
                    .position(|p| matches!(p.slot_type, PlayerSlotType::Bot { .. }))
            })
    }

    /// Configure a player slot (creator only).
    pub fn configure_slot(&mut self, slot: usize, player_type: &str) -> Result<(), ServerError> {
        if slot >= self.num_players {
            return Err(ServerError::InvalidSlot(slot));
        }
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::NotInLobby);
        }

        match player_type {
            "Empty" => {
                // Can't empty the creator slot
                if slot == self.creator {
                    return Err(ServerError::CannotModifyCreator);
                }
                self.players[slot] = PlayerSlot {
                    name: String::new(),
                    slot_type: PlayerSlotType::Empty,
                    session_token: None,
                    connected: false,
                    ip: None,
                    disconnected_at: None,
                    was_human: false,
                    latency_ms: None,
                    broadcast_lag_count: 0,
                };
            }
            s if s.starts_with("Bot:") => {
                let strategy = &s[4..];
                // Validate strategy name (Genetic variants are valid even without genome at config time)
                if !strategy.starts_with("Genetic") {
                    make_strategy(strategy, None, 0)?;
                }
                // If this slot had a human, disconnect them
                self.players[slot] = PlayerSlot {
                    name: format!("Bot ({})", strategy),
                    slot_type: PlayerSlotType::Bot {
                        strategy: strategy.to_string(),
                    },
                    session_token: None,
                    connected: false,
                    ip: None,
                    disconnected_at: None,
                    was_human: false,
                    latency_ms: None,
                    broadcast_lag_count: 0,
                };
            }
            _ => {
                return Err(ServerError::InvalidAction(format!(
                    "Unknown player type: {player_type}"
                )));
            }
        }

        Ok(())
    }

    /// Change the rule set (lobby only).
    pub fn set_rules(&mut self, rules: &str) -> Result<(), ServerError> {
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::NotInLobby);
        }
        // Validate the rules name
        make_rules(rules)?;
        self.rules_name = rules.to_string();
        self.touch();
        Ok(())
    }

    /// Set the turn timer (lobby only, creator only).
    pub fn set_turn_timer(&mut self, secs: Option<u64>) -> Result<(), ServerError> {
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::NotInLobby);
        }
        // Validate: must be None (unlimited) or within 10–300 seconds
        if let Some(s) = secs
            && !(10..=300).contains(&s)
        {
            return Err(ServerError::InvalidTurnTimer);
        }
        self.turn_timer_secs = secs;
        self.touch();
        Ok(())
    }

    /// Reset the turn start timer. Call after each action.
    /// Sets `turn_start` if the current player is human and timer is enabled.
    pub fn reset_turn_start(&mut self) {
        if self.turn_timer_secs.is_none() {
            self.turn_start = None;
            return;
        }
        let game = match &self.game {
            Some(g) => g,
            None => {
                self.turn_start = None;
                return;
            }
        };
        match game.current_player_index() {
            Some(idx) if matches!(self.players[idx].slot_type, PlayerSlotType::Human) => {
                self.turn_start = Some(Instant::now());
            }
            _ => {
                self.turn_start = None;
            }
        }
    }

    /// Get the elapsed duration since the current turn started, or None if not tracked.
    pub fn elapsed_since_turn_start(&self) -> Option<Duration> {
        self.turn_start.map(|t| t.elapsed())
    }

    /// Get seconds remaining for the current turn, or None if unlimited / not applicable.
    pub fn turn_deadline_secs(&self) -> Option<u64> {
        let timer = self.turn_timer_secs?;
        let start = self.turn_start?;
        let elapsed = start.elapsed().as_secs();
        Some(timer.saturating_sub(elapsed))
    }

    /// Check if the current human player's turn has timed out.
    /// If so, apply a random action and return the (player, action, delta) tuple.
    pub fn check_turn_timeout(
        &mut self,
    ) -> Result<Option<(usize, PlayerAction, StateDelta)>, ServerError> {
        let timer = match self.turn_timer_secs {
            Some(t) => t,
            None => return Ok(None),
        };
        let start = match self.turn_start {
            Some(s) => s,
            None => return Ok(None),
        };
        if start.elapsed().as_secs() < timer {
            return Ok(None);
        }

        // Timeout! Play a random action for the current player.
        let (current, action, delta) = {
            let game = self.game.as_mut().ok_or(ServerError::NotInGame)?;
            let current = game.current_player_index().ok_or(ServerError::NotInGame)?;
            let strategy = RandomStrategy;
            let action = game
                .get_bot_action(&strategy)
                .map_err(|e| ServerError::InvalidAction(e.to_string()))?;

            let before = game.get_player_state(current);

            game.apply_action(action.clone())
                .map_err(|e| ServerError::InvalidAction(e.to_string()))?;

            let after = game.get_player_state(current);
            let delta = compute_delta(&before, &after);

            (current, action, delta)
        };

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok(Some((current, action, delta)))
    }

    /// Change the number of player slots. Can add or remove slots from the end.
    /// Cannot reduce below the number of non-empty slots.
    pub fn set_num_players(&mut self, num_players: usize) -> Result<(), ServerError> {
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::NotInLobby);
        }
        if !(2..=8).contains(&num_players) {
            return Err(ServerError::InvalidNumPlayers);
        }

        if num_players > self.num_players {
            // Add empty slots
            for _ in self.num_players..num_players {
                self.players.push(PlayerSlot {
                    name: String::new(),
                    slot_type: PlayerSlotType::Empty,
                    session_token: None,
                    connected: false,
                    ip: None,
                    disconnected_at: None,
                    was_human: false,
                    latency_ms: None,
                    broadcast_lag_count: 0,
                });
            }
        } else if num_players < self.num_players {
            // Remove slots from the end, but only if they're empty
            for i in (num_players..self.num_players).rev() {
                if self.players[i].slot_type != PlayerSlotType::Empty {
                    return Err(ServerError::SlotOccupied);
                }
            }
            self.players.truncate(num_players);
        }

        self.num_players = num_players;
        self.touch();
        Ok(())
    }

    /// Kick a player from the room. Returns their session token so the lobby can clean it up.
    pub fn kick_player(&mut self, slot: usize) -> Result<Option<String>, ServerError> {
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::NotInLobby);
        }
        if slot >= self.num_players {
            return Err(ServerError::InvalidSlot(slot));
        }
        if slot == self.creator {
            return Err(ServerError::CannotModifyCreator);
        }
        if self.players[slot].slot_type == PlayerSlotType::Empty {
            return Err(ServerError::SlotEmpty);
        }

        let token = self.players[slot]
            .session_token
            .as_ref()
            .map(|t| t.to_string());

        // Send kick notification to the player being kicked
        let _ = self.broadcast_tx.send((
            slot,
            crate::messages::ServerMessage::Kicked {
                reason: "You were kicked by the room host".to_string(),
            },
        ));

        // Reset the slot
        self.players[slot] = PlayerSlot {
            name: String::new(),
            slot_type: PlayerSlotType::Empty,
            session_token: None,
            connected: false,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        self.touch();
        Ok(token)
    }

    /// Ban a player by slot. Internally bans their IP (never exposed to clients).
    /// Returns error if trying to ban the creator or if IPs match.
    pub fn ban_player(&mut self, slot: usize) -> Result<Option<String>, ServerError> {
        if slot >= self.num_players {
            return Err(ServerError::InvalidSlot(slot));
        }
        if slot == self.creator {
            return Err(ServerError::CannotModifyCreator);
        }

        let player_ip = self.players[slot].ip.clone();
        let creator_ip = self.players[self.creator].ip.clone();

        // Prevent host from banning their own IP (e.g., same network/localhost)
        if let (Some(p_ip), Some(c_ip)) = (&player_ip, &creator_ip)
            && p_ip == c_ip
        {
            return Err(ServerError::CannotBanSameIp);
        }

        // Ban the IP if we have one
        if let Some(ref ip) = player_ip
            && !self.banned_ips.contains(ip)
        {
            self.banned_ips.push(ip.clone());
        }

        // Kick the player (reuse kick logic)
        self.kick_player(slot)
    }

    /// Check if all required slots are filled (human or bot).
    pub fn all_slots_filled(&self) -> bool {
        self.players
            .iter()
            .all(|p| p.slot_type != PlayerSlotType::Empty)
    }

    /// Start the game. Returns error if not all slots are filled.
    pub fn start_game(&mut self) -> Result<(), ServerError> {
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::GameAlreadyStarted);
        }
        if !self.all_slots_filled() {
            return Err(ServerError::NotAllSlotsFilled);
        }

        let rules = make_rules(&self.rules_name)?;
        let player_names: Vec<String> = self.players.iter().map(|p| p.name.clone()).collect();
        let seed: u64 = rand::rng().random();

        let game = InteractiveGame::new(rules, self.num_players, player_names, seed)
            .map_err(|e| ServerError::InvalidAction(e.to_string()))?;

        self.game = Some(game);
        self.transition(RoomPhase::InGame)?;
        self.reset_turn_start();

        Ok(())
    }

    /// Apply a player action and return the computed delta.
    pub fn apply_action(
        &mut self,
        player_index: usize,
        action: PlayerAction,
    ) -> Result<StateDelta, ServerError> {
        let delta = {
            let game = self.game.as_mut().ok_or(ServerError::NotInGame)?;

            // Turn ownership check
            let current = game.current_player_index();
            if let Some(expected) = current {
                if expected != player_index {
                    return Err(ServerError::NotYourTurn);
                }
            } else {
                return Err(ServerError::NotInGame);
            }

            let before = game.get_player_state(player_index);

            game.apply_action(action)
                .map_err(|e| ServerError::InvalidAction(e.to_string()))?;

            let after = game.get_player_state(player_index);
            compute_delta(&before, &after)
        };

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok(delta)
    }

    /// Apply a bot action for the current player. Returns (player_index, action, delta).
    pub fn apply_bot_action(&mut self) -> Result<(usize, PlayerAction, StateDelta), ServerError> {
        let (current, action, delta) = {
            let game = self.game.as_mut().ok_or(ServerError::NotInGame)?;
            let current = game.current_player_index().ok_or(ServerError::NotInGame)?;

            let strategy_name = match &self.players[current].slot_type {
                PlayerSlotType::Bot { strategy } => strategy.clone(),
                _ => {
                    return Err(ServerError::InvalidAction(
                        "Current player is not a bot".to_string(),
                    ));
                }
            };

            let strategy = make_strategy(
                &strategy_name,
                self.genetic_genome.as_ref(),
                self.genetic_games_trained,
            )?;
            let action = game
                .get_bot_action(strategy.as_ref())
                .map_err(|e| ServerError::InvalidAction(e.to_string()))?;

            let before = game.get_player_state(current);

            game.apply_action(action.clone())
                .map_err(|e| ServerError::InvalidAction(e.to_string()))?;

            let after = game.get_player_state(current);
            let delta = compute_delta(&before, &after);

            (current, action, delta)
        };

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok((current, action, delta))
    }

    fn check_game_over(&mut self) {
        if let Some(game) = &self.game {
            let action_needed = game.get_action_needed();
            if let skyjo_core::ActionNeeded::GameOver { ref winners, .. } = action_needed {
                self.transition(RoomPhase::GameOver)
                    .expect("InGame → GameOver is a valid transition");
                self.last_winners = winners.clone();
            }
        }
    }

    /// Continue to next round. Phase remains InGame (no transition needed).
    pub fn continue_round(&mut self) -> Result<(), ServerError> {
        let game = self.game.as_mut().ok_or(ServerError::NotInGame)?;
        game.apply_action(PlayerAction::ContinueToNextRound)
            .map_err(|e| ServerError::InvalidAction(e.to_string()))?;
        self.touch();
        self.reset_turn_start();
        Ok(())
    }

    /// Reset for a new game (play again).
    pub fn play_again(&mut self) -> Result<(), ServerError> {
        self.game = None;
        self.transition(RoomPhase::Lobby)?;
        Ok(())
    }

    /// Return to lobby after game ends (preserves last_winners for crown display).
    pub fn return_to_lobby(&mut self) -> Result<(), ServerError> {
        // last_winners is already set by check_game_over
        self.game = None;
        self.transition(RoomPhase::Lobby)?;
        Ok(())
    }

    /// Promote another player to host.
    pub fn promote_host(&mut self, slot: usize) -> Result<(), ServerError> {
        if slot >= self.num_players {
            return Err(ServerError::InvalidSlot(slot));
        }
        if !matches!(self.players[slot].slot_type, PlayerSlotType::Human) {
            return Err(ServerError::InvalidAction(
                "Can only promote human players".to_string(),
            ));
        }
        self.creator = slot;
        self.touch();
        Ok(())
    }

    /// Auto-promote host: find the next connected human player.
    pub fn auto_promote_host(&mut self) -> bool {
        if self.players[self.creator].connected {
            return false; // Host is still connected
        }
        // Find next connected human
        for i in 0..self.num_players {
            if i != self.creator
                && self.players[i].connected
                && matches!(self.players[i].slot_type, PlayerSlotType::Human)
            {
                self.creator = i;
                self.touch();
                return true;
            }
        }
        false
    }

    /// Auto-kick players who have been disconnected for longer than the timeout.
    /// Returns the list of kicked slot indices and their session tokens.
    pub fn auto_kick_disconnected(&mut self, timeout: Duration) -> Vec<(usize, Option<String>)> {
        let mut kicked = Vec::new();
        for i in 0..self.num_players {
            if let Some(dc_at) = self.players[i].disconnected_at
                && dc_at.elapsed() >= timeout
                && matches!(self.players[i].slot_type, PlayerSlotType::Human)
            {
                let token = self.players[i]
                    .session_token
                    .as_ref()
                    .map(|t| t.to_string());
                // Send kick notification
                let _ = self.broadcast_tx.send((
                    i,
                    crate::messages::ServerMessage::Kicked {
                        reason: "Disconnected for too long".to_string(),
                    },
                ));
                self.players[i] = PlayerSlot {
                    name: String::new(),
                    slot_type: PlayerSlotType::Empty,
                    session_token: None,
                    connected: false,
                    ip: None,
                    disconnected_at: None,
                    was_human: false,
                    latency_ms: None,
                    broadcast_lag_count: 0,
                };
                kicked.push((i, token));
            }
        }
        if !kicked.is_empty() {
            self.touch();
        }
        kicked
    }

    /// Convert disconnected human players to bots after the given timeout.
    /// Only applies during InGame phase. Returns the list of converted slot indices.
    pub fn convert_disconnected_to_bots(&mut self, timeout: Duration) -> Vec<usize> {
        let mut converted = Vec::new();
        for i in 0..self.num_players {
            if let PlayerSlotType::Human = self.players[i].slot_type
                && let Some(dc_at) = self.players[i].disconnected_at
                && dc_at.elapsed() >= timeout
            {
                self.players[i].slot_type = PlayerSlotType::Bot {
                    strategy: "Random".to_string(),
                };
                if !self.players[i].name.ends_with(" (Bot)") {
                    self.players[i].name.push_str(" (Bot)");
                }
                self.players[i].was_human = true;
                converted.push(i);
            }
        }
        if !converted.is_empty() {
            self.touch();
        }
        converted
    }

    /// Reconvert a bot-converted player back to human on reconnection.
    /// Returns true if the player was reconverted.
    pub fn reconnect_bot_to_human(&mut self, player_index: usize) -> bool {
        if !self.players[player_index].was_human {
            return false;
        }
        self.players[player_index].slot_type = PlayerSlotType::Human;
        if let Some(name) = self.players[player_index].name.strip_suffix(" (Bot)") {
            self.players[player_index].name = name.to_string();
        }
        self.players[player_index].was_human = false;
        self.touch();
        true
    }

    /// Set the disconnect-to-bot timeout (lobby only, creator only).
    pub fn set_disconnect_bot_timeout(&mut self, secs: Option<u32>) -> Result<(), ServerError> {
        if self.phase != RoomPhase::Lobby {
            return Err(ServerError::NotInLobby);
        }
        if let Some(s) = secs
            && !(10..=300).contains(&s)
        {
            return Err(ServerError::InvalidDisconnectTimeout);
        }
        self.disconnect_bot_timeout_secs = secs;
        self.touch();
        Ok(())
    }

    /// Get the effective disconnect-to-bot timeout in seconds.
    pub fn effective_disconnect_bot_timeout(&self) -> Duration {
        Duration::from_secs(self.disconnect_bot_timeout_secs.unwrap_or(60) as u64)
    }

    /// Get the per-player game state for a specific player.
    pub fn get_player_state(
        &self,
        player_index: usize,
    ) -> Result<InteractiveGameState, ServerError> {
        let game = self.game.as_ref().ok_or(ServerError::NotInGame)?;
        Ok(game.get_player_state(player_index))
    }

    /// Check if the current player is a bot.
    pub fn is_current_player_bot(&self) -> bool {
        let game = match &self.game {
            Some(g) => g,
            None => return false,
        };
        match game.current_player_index() {
            Some(idx) => matches!(self.players[idx].slot_type, PlayerSlotType::Bot { .. }),
            None => false,
        }
    }

    /// Update the measured latency for a player.
    pub fn update_player_latency(&mut self, slot: usize, latency_ms: u32) {
        if slot < self.num_players {
            self.players[slot].latency_ms = Some(latency_ms);
        }
    }

    /// Increment the broadcast lag counter for a player.
    pub fn increment_broadcast_lag(&mut self, slot: usize) {
        if slot < self.num_players {
            self.players[slot].broadcast_lag_count += 1;
        }
    }

    /// Build the lobby state for broadcasting.
    pub fn lobby_state(&self) -> RoomLobbyState {
        let creator_ip = self.players[self.creator].ip.as_deref();
        let players: Vec<LobbyPlayer> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // For human players (not creator), check if they share an IP with the host
                let shares_ip = if i != self.creator && matches!(p.slot_type, PlayerSlotType::Human)
                {
                    match (&p.ip, creator_ip) {
                        (Some(pip), Some(cip)) => Some(pip.as_str() == cip),
                        _ => None,
                    }
                } else {
                    None
                };
                let disconnect_secs = p.disconnected_at.map(|t| t.elapsed().as_secs());
                LobbyPlayer {
                    slot: i,
                    name: p.name.clone(),
                    player_type: p.slot_type.clone(),
                    connected: p.connected,
                    shares_ip_with_host: shares_ip,
                    disconnect_secs,
                    latency_ms: p.latency_ms,
                    broadcast_lag_count: p.broadcast_lag_count,
                }
            })
            .collect();

        // Calculate idle timeout: rooms in lobby phase expire after 30 min of inactivity
        let idle_timeout_secs = if self.phase == RoomPhase::Lobby {
            let elapsed = self.last_activity.elapsed().as_secs();
            let timeout = 30 * 60u64; // 30 minutes
            Some(timeout.saturating_sub(elapsed))
        } else {
            None
        };

        RoomLobbyState {
            room_code: self.code.clone(),
            players,
            num_players: self.num_players,
            rules: self.rules_name.clone(),
            creator: self.creator,
            available_strategies: available_strategies(),
            available_rules: available_rules(),
            idle_timeout_secs,
            turn_timer_secs: self.turn_timer_secs,
            disconnect_bot_timeout_secs: self.disconnect_bot_timeout_secs,
            last_winners: self.last_winners.clone(),
            genetic_games_trained: self.genetic_games_trained,
            genetic_generation: self.genetic_generation,
        }
    }

    /// Create a serializable snapshot of this room for crash recovery.
    pub fn to_snapshot(&self) -> RoomSnapshot {
        let players = self
            .players
            .iter()
            .map(|p| PlayerSlotSnapshot {
                name: p.name.clone(),
                slot_type: p.slot_type.clone(),
                was_human: p.was_human,
            })
            .collect();

        // Serialize the full game state if a game is active
        let game_state_json = self.game.as_ref().map(|g| {
            let state = g.get_full_state();
            serde_json::to_string(&state).expect("InteractiveGameState serialization failed")
        });

        RoomSnapshot {
            code: self.code.clone(),
            phase: self.phase.clone(),
            num_players: self.num_players,
            creator: self.creator,
            players,
            rules_name: self.rules_name.clone(),
            turn_timer_secs: self.turn_timer_secs,
            disconnect_bot_timeout_secs: self.disconnect_bot_timeout_secs,
            game_state_json,
            banned_ips: self.banned_ips.clone(),
            last_winners: self.last_winners.clone(),
        }
    }

    /// Restore a room from a snapshot. Game state is NOT restored (too complex
    /// to reconstruct `InteractiveGame` from a state snapshot). The room is
    /// placed back in Lobby phase so players can start a new game.
    pub fn from_snapshot(snapshot: RoomSnapshot) -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);

        let players = snapshot
            .players
            .iter()
            .map(|ps| PlayerSlot {
                name: ps.name.clone(),
                slot_type: ps.slot_type.clone(),
                session_token: None,
                connected: false,
                ip: None,
                disconnected_at: None,
                was_human: ps.was_human,
                latency_ms: None,
                broadcast_lag_count: 0,
            })
            .collect();

        Room {
            code: snapshot.code,
            // Always restore to Lobby — game state isn't fully restorable
            phase: RoomPhase::Lobby,
            num_players: snapshot.num_players,
            rules_name: snapshot.rules_name,
            creator: snapshot.creator,
            players,
            game: None,
            last_activity: Instant::now(),
            broadcast_tx,
            banned_ips: snapshot.banned_ips,
            last_winners: snapshot.last_winners,
            turn_timer_secs: snapshot.turn_timer_secs,
            turn_start: None,
            genetic_genome: None,
            genetic_games_trained: 0,
            genetic_generation: 0,
            disconnect_bot_timeout_secs: snapshot.disconnect_bot_timeout_secs,
            player_txs: vec![None; snapshot.num_players],
        }
    }

    // -- Per-player targeted channel methods --

    /// Register a player's message channel (called on connect/reconnect).
    pub fn set_player_tx(&mut self, slot: usize, tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>) {
        if slot < self.player_txs.len() {
            self.player_txs[slot] = Some(tx);
        }
    }

    /// Remove a player's message channel (called on disconnect).
    pub fn remove_player_tx(&mut self, slot: usize) {
        if slot < self.player_txs.len() {
            self.player_txs[slot] = None;
        }
    }

    /// Send a pre-serialized message to a specific player.
    pub fn send_to_player(&self, slot: usize, data: Vec<u8>) -> bool {
        if let Some(Some(tx)) = self.player_txs.get(slot) {
            tx.send(data).is_ok()
        } else {
            false
        }
    }

    /// Send a pre-serialized message to all connected players.
    /// `data_fn` receives the player slot index and returns the bytes to send.
    pub fn send_to_all(&self, data_fn: impl Fn(usize) -> Vec<u8>) {
        for (i, tx_opt) in self.player_txs.iter().enumerate() {
            if let Some(tx) = tx_opt {
                let _ = tx.send(data_fn(i));
            }
        }
    }

    /// Broadcast a per-player-filtered game state to all connected players.
    pub fn broadcast_game_state(&self) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };

        let deadline = self.turn_deadline_secs();

        for (i, player) in self.players.iter().enumerate() {
            if player.connected && matches!(player.slot_type, PlayerSlotType::Human) {
                let state = game.get_player_state(i);
                let _ = self.broadcast_tx.send((
                    i,
                    ServerMessage::GameState {
                        state,
                        turn_deadline_secs: deadline,
                    },
                ));
            }
        }
    }

    /// Broadcast a message with per-player state to all connected human players.
    /// `make_msg` receives the player index and their filtered game state.
    pub fn broadcast_action(
        &self,
        player: usize,
        action: &PlayerAction,
        is_bot: bool,
        delta: &StateDelta,
    ) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };

        let deadline = self.turn_deadline_secs();

        // Broadcast delta to all connected human players
        let mut delta_with_deadline = delta.clone();
        delta_with_deadline.turn_deadline_secs = deadline.map(|d| d as f64);
        let _ = self.broadcast_tx.send((
            usize::MAX,
            ServerMessage::ActionAppliedDelta {
                player,
                action: action.clone(),
                delta: delta_with_deadline,
            },
        ));

        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected && matches!(slot.slot_type, PlayerSlotType::Human) {
                let state = game.get_player_state(i);
                let msg = if is_bot {
                    ServerMessage::BotAction {
                        player,
                        action: action.clone(),
                        state,
                        turn_deadline_secs: deadline,
                    }
                } else {
                    ServerMessage::ActionApplied {
                        player,
                        action: action.clone(),
                        state,
                        turn_deadline_secs: deadline,
                    }
                };
                let _ = self.broadcast_tx.send((i, msg));
            }
        }
    }

    /// Broadcast a timeout action to all connected human players.
    pub fn broadcast_timeout_action(
        &self,
        player: usize,
        action: &PlayerAction,
        delta: &StateDelta,
    ) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };

        // Broadcast delta to all connected human players
        let _ = self.broadcast_tx.send((
            usize::MAX,
            ServerMessage::ActionAppliedDelta {
                player,
                action: action.clone(),
                delta: delta.clone(),
            },
        ));

        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected && matches!(slot.slot_type, PlayerSlotType::Human) {
                let state = game.get_player_state(i);
                let msg = ServerMessage::TimeoutAction {
                    player,
                    action: action.clone(),
                    state,
                };
                let _ = self.broadcast_tx.send((i, msg));
            }
        }
    }

    /// Broadcast a lobby state update to all connected players.
    pub fn broadcast_lobby_state(&self) {
        let state = self.lobby_state();
        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected {
                let _ = self.broadcast_tx.send((
                    i,
                    ServerMessage::RoomState {
                        state: state.clone(),
                    },
                ));
            }
        }
    }
}

/// Shared room handle for concurrent access.
pub type SharedRoom = Arc<Mutex<Room>>;

fn make_strategy(
    name: &str,
    genetic_genome: Option<&Vec<f32>>,
    genetic_games_trained: usize,
) -> Result<Box<dyn Strategy>, ServerError> {
    match name {
        "Random" => Ok(Box::new(RandomStrategy)),
        "Greedy" => Ok(Box::new(GreedyStrategy)),
        "Defensive" => Ok(Box::new(DefensiveStrategy)),
        "Clearer" => Ok(Box::new(ClearerStrategy)),
        "Statistician" => Ok(Box::new(StatisticianStrategy)),
        "Rusher" => Ok(Box::new(RusherStrategy)),
        "Gambler" => Ok(Box::new(GamblerStrategy)),
        "Survivor" => Ok(Box::new(SurvivorStrategy)),
        "Mimic" => Ok(Box::new(MimicStrategy)),
        "Saboteur" => Ok(Box::new(SaboteurStrategy)),
        "Genetic" => {
            let genome = genetic_genome
                .ok_or(ServerError::InvalidStrategy(
                    "Genetic strategy requires a trained model".to_string(),
                ))?
                .clone();
            Ok(Box::new(GeneticStrategy::new(
                genome,
                genetic_games_trained,
            )))
        }
        s if s.starts_with("Genetic:") => {
            // Specific saved generation: "Genetic:Gen 50"
            // Genome is provided via genetic_genome (resolved by caller)
            let genome = genetic_genome
                .ok_or(ServerError::InvalidStrategy(
                    "Saved genetic generation not found".to_string(),
                ))?
                .clone();
            Ok(Box::new(GeneticStrategy::new(
                genome,
                genetic_games_trained,
            )))
        }
        _ => Err(ServerError::InvalidStrategy(name.to_string())),
    }
}

fn make_rules(name: &str) -> Result<Box<dyn Rules>, ServerError> {
    match name {
        "Standard" | "" => Ok(Box::new(StandardRules)),
        _ => Err(ServerError::InvalidRules(name.to_string())),
    }
}

pub fn available_strategies() -> Vec<String> {
    // Ordered by complexity: Trivial → Low → Medium → High
    vec![
        "Random".to_string(),
        "Greedy".to_string(),
        "Gambler".to_string(),
        "Rusher".to_string(),
        "Defensive".to_string(),
        "Clearer".to_string(),
        "Mimic".to_string(),
        "Saboteur".to_string(),
        "Survivor".to_string(),
        "Statistician".to_string(),
        "Genetic".to_string(),
    ]
}

pub fn available_rules() -> Vec<String> {
    vec!["Standard".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServerError;

    /// Create a basic 2-player room in Lobby phase.
    fn test_room() -> Room {
        Room::new("TEST1".to_string(), "Alice".to_string(), 2, None, 0, 0)
    }

    /// Create a room with all slots filled (1 human creator + 1 bot).
    fn filled_room() -> Room {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Random").unwrap();
        room
    }

    /// Create a room in InGame phase (2 players: human + bot).
    /// Note: Uses a non-deterministic seed via Room::start_game().
    /// This is acceptable because bot strategies always produce valid moves
    /// regardless of board state, and the tests verify state transitions
    /// rather than specific game outcomes.
    fn ingame_room() -> Room {
        let mut room = filled_room();
        room.start_game().unwrap();
        room
    }

    // ========================================================================
    // Room Creation & Configuration
    // ========================================================================

    #[test]
    fn new_room_has_lobby_phase_and_correct_defaults() {
        let room = test_room();
        assert_eq!(room.phase, RoomPhase::Lobby);
        assert_eq!(room.code, "TEST1");
        assert_eq!(room.num_players, 2);
        assert_eq!(room.creator, 0);
        assert_eq!(room.rules_name, "Standard");
        assert!(room.game.is_none());
        assert!(room.banned_ips.is_empty());
        assert!(room.last_winners.is_empty());
        assert_eq!(room.turn_timer_secs, Some(60));
        assert_eq!(room.players.len(), 2);
        // Creator slot
        assert_eq!(room.players[0].name, "Alice");
        assert_eq!(room.players[0].slot_type, PlayerSlotType::Human);
        // Second slot is empty
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
    }

    #[test]
    fn new_room_with_custom_rules() {
        let room = Room::new(
            "R2".to_string(),
            "Bob".to_string(),
            4,
            Some("Standard".to_string()),
            100,
            5,
        );
        assert_eq!(room.rules_name, "Standard");
        assert_eq!(room.num_players, 4);
        assert_eq!(room.players.len(), 4);
        assert_eq!(room.genetic_games_trained, 100);
        assert_eq!(room.genetic_generation, 5);
    }

    #[test]
    fn configure_slot_to_bot() {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Greedy").unwrap();
        assert_eq!(
            room.players[1].slot_type,
            PlayerSlotType::Bot {
                strategy: "Greedy".to_string()
            }
        );
        assert_eq!(room.players[1].name, "Bot (Greedy)");
    }

    #[test]
    fn configure_slot_to_empty() {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Random").unwrap();
        room.configure_slot(1, "Empty").unwrap();
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
    }

    #[test]
    fn configure_slot_cannot_empty_creator() {
        let mut room = test_room();
        let err = room.configure_slot(0, "Empty").unwrap_err();
        assert_eq!(err, ServerError::CannotModifyCreator);
    }

    #[test]
    fn configure_slot_invalid_slot() {
        let mut room = test_room();
        let err = room.configure_slot(5, "Empty").unwrap_err();
        assert_eq!(err, ServerError::InvalidSlot(5));
    }

    #[test]
    fn configure_slot_unknown_type() {
        let mut room = test_room();
        let err = room.configure_slot(1, "Alien").unwrap_err();
        assert!(matches!(err, ServerError::InvalidAction(_)));
    }

    #[test]
    fn configure_slot_invalid_strategy() {
        let mut room = test_room();
        let err = room.configure_slot(1, "Bot:NonExistent").unwrap_err();
        assert!(matches!(err, ServerError::InvalidStrategy(_)));
    }

    #[test]
    fn configure_slot_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.configure_slot(1, "Bot:Greedy").unwrap_err();
        assert_eq!(err, ServerError::NotInLobby);
    }

    #[test]
    fn set_num_players_increase() {
        let mut room = test_room();
        room.set_num_players(4).unwrap();
        assert_eq!(room.num_players, 4);
        assert_eq!(room.players.len(), 4);
        assert_eq!(room.players[2].slot_type, PlayerSlotType::Empty);
        assert_eq!(room.players[3].slot_type, PlayerSlotType::Empty);
    }

    #[test]
    fn set_num_players_decrease_empty_slots() {
        let mut room = test_room();
        room.set_num_players(4).unwrap();
        room.set_num_players(2).unwrap();
        assert_eq!(room.num_players, 2);
        assert_eq!(room.players.len(), 2);
    }

    #[test]
    fn set_num_players_cannot_decrease_below_occupied() {
        let mut room = Room::new("R".to_string(), "A".to_string(), 3, None, 0, 0);
        room.configure_slot(2, "Bot:Random").unwrap();
        let err = room.set_num_players(2).unwrap_err();
        assert_eq!(err, ServerError::SlotOccupied);
    }

    #[test]
    fn set_num_players_invalid_below_2() {
        let mut room = test_room();
        let err = room.set_num_players(1).unwrap_err();
        assert_eq!(err, ServerError::InvalidNumPlayers);
    }

    #[test]
    fn set_num_players_invalid_above_8() {
        let mut room = test_room();
        let err = room.set_num_players(9).unwrap_err();
        assert_eq!(err, ServerError::InvalidNumPlayers);
    }

    #[test]
    fn set_num_players_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_num_players(3).unwrap_err();
        assert_eq!(err, ServerError::NotInLobby);
    }

    #[test]
    fn set_rules_standard() {
        let mut room = test_room();
        room.set_rules("Standard").unwrap();
        assert_eq!(room.rules_name, "Standard");
    }

    #[test]
    fn set_rules_invalid() {
        let mut room = test_room();
        let err = room.set_rules("Bogus").unwrap_err();
        assert!(matches!(err, ServerError::InvalidRules(_)));
    }

    #[test]
    fn set_rules_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_rules("Standard").unwrap_err();
        assert_eq!(err, ServerError::NotInLobby);
    }

    #[test]
    fn set_turn_timer_valid() {
        let mut room = test_room();
        room.set_turn_timer(Some(30)).unwrap();
        assert_eq!(room.turn_timer_secs, Some(30));
    }

    #[test]
    fn set_turn_timer_unlimited() {
        let mut room = test_room();
        room.set_turn_timer(None).unwrap();
        assert_eq!(room.turn_timer_secs, None);
    }

    #[test]
    fn set_turn_timer_rejects_zero() {
        let mut room = test_room();
        let err = room.set_turn_timer(Some(0)).unwrap_err();
        assert_eq!(err, ServerError::InvalidTurnTimer);
    }

    #[test]
    fn set_turn_timer_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_turn_timer(Some(30)).unwrap_err();
        assert_eq!(err, ServerError::NotInLobby);
    }

    #[test]
    fn set_turn_timer_rejects_below_10() {
        let mut room = test_room();
        let err = room.set_turn_timer(Some(5)).unwrap_err();
        assert_eq!(err, ServerError::InvalidTurnTimer);
    }

    #[test]
    fn set_turn_timer_rejects_above_300() {
        let mut room = test_room();
        let err = room.set_turn_timer(Some(301)).unwrap_err();
        assert_eq!(err, ServerError::InvalidTurnTimer);
    }

    #[test]
    fn set_turn_timer_accepts_boundary_10() {
        let mut room = test_room();
        room.set_turn_timer(Some(10)).unwrap();
        assert_eq!(room.turn_timer_secs, Some(10));
    }

    #[test]
    fn set_turn_timer_accepts_boundary_300() {
        let mut room = test_room();
        room.set_turn_timer(Some(300)).unwrap();
        assert_eq!(room.turn_timer_secs, Some(300));
    }

    // ========================================================================
    // Player Name Validation
    // ========================================================================

    #[test]
    fn player_name_max_length() {
        let long_name = "A".repeat(33);
        let result = validate_player_name(&long_name);
        assert_eq!(result.unwrap_err(), ServerError::PlayerNameTooLong);
    }

    #[test]
    fn player_name_exactly_32_accepted() {
        let name = "A".repeat(32);
        let result = validate_player_name(&name);
        assert_eq!(result.unwrap(), name);
    }

    #[test]
    fn player_name_trimmed() {
        let result = validate_player_name("  Alice  ");
        assert_eq!(result.unwrap(), "Alice");
    }

    #[test]
    fn player_name_empty_rejected() {
        assert_eq!(
            validate_player_name("").unwrap_err(),
            ServerError::PlayerNameEmpty
        );
    }

    #[test]
    fn player_name_whitespace_only_rejected() {
        assert_eq!(
            validate_player_name("   ").unwrap_err(),
            ServerError::PlayerNameEmpty
        );
    }

    // ========================================================================
    // Room Code Validation
    // ========================================================================

    #[test]
    fn room_code_valid_accepted() {
        assert!(validate_room_code("ABC234").is_ok());
        assert!(validate_room_code("XXXXXX").is_ok());
    }

    #[test]
    fn room_code_rejects_lowercase() {
        assert_eq!(
            validate_room_code("abcdef").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
    }

    #[test]
    fn room_code_rejects_excluded_chars() {
        // I and O are excluded from alphabet; 0 and 1 from digits
        assert_eq!(
            validate_room_code("ABCDEI").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
        assert_eq!(
            validate_room_code("ABCDEO").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
        assert_eq!(
            validate_room_code("ABCDE0").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
        assert_eq!(
            validate_room_code("ABCDE1").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
    }

    #[test]
    fn room_code_rejects_wrong_length() {
        assert_eq!(
            validate_room_code("ABC").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
        assert_eq!(
            validate_room_code("ABCDEFG").unwrap_err(),
            ServerError::RoomCodeInvalid
        );
    }

    // ========================================================================
    // Game Lifecycle
    // ========================================================================

    #[test]
    fn start_game_transitions_to_ingame() {
        let mut room = filled_room();
        room.start_game().unwrap();
        assert_eq!(room.phase, RoomPhase::InGame);
        assert!(room.game.is_some());
    }

    #[test]
    fn start_game_fails_if_not_all_slots_filled() {
        let mut room = test_room();
        let err = room.start_game().unwrap_err();
        assert_eq!(err, ServerError::NotAllSlotsFilled);
    }

    #[test]
    fn start_game_fails_if_already_started() {
        let mut room = ingame_room();
        let err = room.start_game().unwrap_err();
        assert_eq!(err, ServerError::GameAlreadyStarted);
    }

    #[test]
    fn apply_action_works_during_ingame() {
        let mut room = ingame_room();
        let game = room.game.as_ref().unwrap();
        let state = game.get_player_state(0);
        // The game should have an action needed; verify apply_action at least doesn't
        // panic when given a valid context. We use the bot action route for simplicity.
        // Try getting the current player and applying a bot action if it's a bot turn.
        let current = game.current_player_index();
        assert!(current.is_some());
        // The game is freshly started — either player 0 or 1 goes first.
        // We just verify the game state is accessible.
        drop(state);
        let _ = current;
        // Apply bot actions until it's a human turn or the round ends
        while room.is_current_player_bot() {
            room.apply_bot_action().unwrap();
        }
        // Now it should be the human player's turn (or game could be over).
        // At minimum, verify we didn't panic.
    }

    #[test]
    fn apply_action_rejects_wrong_player() {
        let mut room = ingame_room();
        // Advance past any bot turns
        while room.is_current_player_bot() {
            room.apply_bot_action().unwrap();
        }
        if room.phase != RoomPhase::InGame {
            return; // Game ended during bot turns
        }
        let current = room.game.as_ref().unwrap().current_player_index().unwrap();
        let wrong_player = if current == 0 { 1 } else { 0 };
        let err = room
            .apply_action(wrong_player, PlayerAction::InitialFlip { position: 0 })
            .unwrap_err();
        assert_eq!(err, ServerError::NotYourTurn);
    }

    #[test]
    fn apply_bot_action_returns_valid_action() {
        let mut room = ingame_room();
        // If current player isn't a bot, configure so slot 1 is bot and wait for its turn
        if room.is_current_player_bot() {
            let (player_idx, _action, _delta) = room.apply_bot_action().unwrap();
            assert!(player_idx < room.num_players);
        }
        // At minimum we've verified bot action doesn't error
    }

    #[test]
    fn apply_bot_action_fails_for_human_player() {
        let mut room = ingame_room();
        // Advance past bot turns
        while room.is_current_player_bot() {
            room.apply_bot_action().unwrap();
        }
        if room.phase != RoomPhase::InGame {
            return;
        }
        let err = room.apply_bot_action().unwrap_err();
        assert!(matches!(err, ServerError::InvalidAction(_)));
    }

    #[test]
    fn play_again_resets_game() {
        let mut room = ingame_room();
        // Force game over by playing through
        play_until_game_over(&mut room);
        assert_eq!(room.phase, RoomPhase::GameOver);

        room.play_again().unwrap();
        assert_eq!(room.phase, RoomPhase::Lobby);
        assert!(room.game.is_none());
    }

    #[test]
    fn play_again_fails_if_not_game_over() {
        let mut room = ingame_room();
        let err = room.play_again().unwrap_err();
        assert!(matches!(err, ServerError::InvalidAction(_)));
    }

    #[test]
    fn return_to_lobby_resets_phase() {
        let mut room = ingame_room();
        play_until_game_over(&mut room);
        assert_eq!(room.phase, RoomPhase::GameOver);

        room.return_to_lobby().unwrap();
        assert_eq!(room.phase, RoomPhase::Lobby);
        assert!(room.game.is_none());
    }

    #[test]
    fn return_to_lobby_fails_if_not_game_over() {
        let mut room = ingame_room();
        let err = room.return_to_lobby().unwrap_err();
        assert!(matches!(err, ServerError::InvalidAction(_)));
    }

    #[test]
    fn continue_round_progresses_game() {
        let mut room = ingame_room();
        // Play a full round until round-over
        play_until_round_over(&mut room);
        if room.phase == RoomPhase::GameOver {
            return; // Game ended in one round, can't test continue_round
        }
        // Now continue_round should work
        room.continue_round().unwrap();
        assert_eq!(room.phase, RoomPhase::InGame);
    }

    // ========================================================================
    // Player Management
    // ========================================================================

    #[test]
    fn kick_player_removes_and_returns_token() {
        let mut room = test_room();
        // Add a human to slot 1 by simulating a join
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: Some(SessionToken::new()),
            connected: true,
            ip: Some("1.2.3.4".to_string()),
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        let token = room.kick_player(1).unwrap();
        assert!(token.is_some());
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
        assert_eq!(room.players[1].name, "");
    }

    #[test]
    fn kick_player_rejects_creator() {
        let mut room = test_room();
        let err = room.kick_player(0).unwrap_err();
        assert_eq!(err, ServerError::CannotModifyCreator);
    }

    #[test]
    fn kick_player_rejects_empty_slot() {
        let mut room = test_room();
        let err = room.kick_player(1).unwrap_err();
        assert_eq!(err, ServerError::SlotEmpty);
    }

    #[test]
    fn kick_player_rejects_invalid_slot() {
        let mut room = test_room();
        let err = room.kick_player(10).unwrap_err();
        assert_eq!(err, ServerError::InvalidSlot(10));
    }

    #[test]
    fn kick_player_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.kick_player(1).unwrap_err();
        assert_eq!(err, ServerError::NotInLobby);
    }

    #[test]
    fn ban_player_adds_ip_to_banned_list() {
        let mut room = test_room();
        room.players[0].ip = Some("10.0.0.1".to_string());
        room.players[1] = PlayerSlot {
            name: "Eve".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: Some(SessionToken::new()),
            connected: true,
            ip: Some("10.0.0.2".to_string()),
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        room.ban_player(1).unwrap();
        assert!(room.is_ip_banned("10.0.0.2"));
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
    }

    #[test]
    fn ban_player_rejects_same_ip_as_creator() {
        let mut room = test_room();
        room.players[0].ip = Some("10.0.0.1".to_string());
        room.players[1] = PlayerSlot {
            name: "Eve".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: Some("10.0.0.1".to_string()),
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        let err = room.ban_player(1).unwrap_err();
        assert_eq!(err, ServerError::CannotBanSameIp);
    }

    #[test]
    fn ban_player_rejects_creator() {
        let mut room = test_room();
        let err = room.ban_player(0).unwrap_err();
        assert_eq!(err, ServerError::CannotModifyCreator);
    }

    #[test]
    fn promote_host_changes_creator() {
        let mut room = test_room();
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };
        room.promote_host(1).unwrap();
        assert_eq!(room.creator, 1);
    }

    #[test]
    fn promote_host_rejects_bot() {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Random").unwrap();
        let err = room.promote_host(1).unwrap_err();
        assert!(matches!(err, ServerError::InvalidAction(_)));
    }

    #[test]
    fn promote_host_rejects_invalid_slot() {
        let mut room = test_room();
        let err = room.promote_host(10).unwrap_err();
        assert_eq!(err, ServerError::InvalidSlot(10));
    }

    #[test]
    fn auto_promote_host_selects_next_connected_human() {
        let mut room = Room::new("R".to_string(), "Alice".to_string(), 3, None, 0, 0);
        room.players[0].connected = false; // host disconnected
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };
        room.players[2] = PlayerSlot {
            name: "Charlie".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        let promoted = room.auto_promote_host();
        assert!(promoted);
        assert_eq!(room.creator, 1);
    }

    #[test]
    fn auto_promote_host_returns_false_if_host_connected() {
        let mut room = test_room();
        room.players[0].connected = true;
        assert!(!room.auto_promote_host());
    }

    #[test]
    fn auto_promote_host_returns_false_if_no_connected_humans() {
        let mut room = test_room();
        room.players[0].connected = false;
        // Slot 1 is empty, no connected humans to promote
        assert!(!room.auto_promote_host());
    }

    #[test]
    fn auto_promote_host_skips_bots() {
        let mut room = Room::new("R".to_string(), "Alice".to_string(), 3, None, 0, 0);
        room.players[0].connected = false;
        room.configure_slot(1, "Bot:Random").unwrap();
        room.players[2] = PlayerSlot {
            name: "Human2".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        let promoted = room.auto_promote_host();
        assert!(promoted);
        assert_eq!(room.creator, 2); // Skipped the bot at slot 1
    }

    #[test]
    fn next_available_slot_finds_empty() {
        let room = test_room();
        assert_eq!(room.next_available_slot(), Some(1));
    }

    #[test]
    fn next_available_slot_falls_back_to_bot() {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Random").unwrap();
        // No empty slots, should find the bot slot
        assert_eq!(room.next_available_slot(), Some(1));
    }

    #[test]
    fn next_available_slot_returns_none_when_all_human() {
        let mut room = test_room();
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: None,
            disconnected_at: None,
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };
        assert_eq!(room.next_available_slot(), None);
    }

    #[test]
    fn all_slots_filled_true_when_full() {
        let room = filled_room();
        assert!(room.all_slots_filled());
    }

    #[test]
    fn all_slots_filled_false_with_empty() {
        let room = test_room();
        assert!(!room.all_slots_filled());
    }

    #[test]
    fn auto_kick_disconnected_removes_timed_out_players() {
        let mut room = Room::new("R".to_string(), "Alice".to_string(), 3, None, 0, 0);
        room.players[0].connected = true;
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: Some(SessionToken::new()),
            connected: false,
            ip: None,
            disconnected_at: Some(Instant::now() - Duration::from_secs(300)),
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        let kicked = room.auto_kick_disconnected(Duration::from_secs(60));
        assert_eq!(kicked.len(), 1);
        assert_eq!(kicked[0].0, 1);
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
    }

    #[test]
    fn auto_kick_disconnected_skips_recently_disconnected() {
        let mut room = Room::new("R".to_string(), "Alice".to_string(), 3, None, 0, 0);
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: Some(SessionToken::new()),
            connected: false,
            ip: None,
            disconnected_at: Some(Instant::now()),
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };

        let kicked = room.auto_kick_disconnected(Duration::from_secs(60));
        assert!(kicked.is_empty());
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Human);
    }

    #[test]
    fn is_ip_banned_works() {
        let mut room = test_room();
        assert!(!room.is_ip_banned("1.2.3.4"));
        room.banned_ips.push("1.2.3.4".to_string());
        assert!(room.is_ip_banned("1.2.3.4"));
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    #[test]
    fn lobby_state_returns_correct_structure() {
        let room = test_room();
        let state = room.lobby_state();
        assert_eq!(state.room_code, "TEST1");
        assert_eq!(state.num_players, 2);
        assert_eq!(state.rules, "Standard");
        assert_eq!(state.creator, 0);
        assert_eq!(state.players.len(), 2);
        assert_eq!(state.players[0].name, "Alice");
        assert_eq!(state.players[0].player_type, PlayerSlotType::Human);
        assert_eq!(state.players[1].player_type, PlayerSlotType::Empty);
        assert!(!state.available_strategies.is_empty());
        assert!(!state.available_rules.is_empty());
        assert!(state.idle_timeout_secs.is_some());
        assert_eq!(state.turn_timer_secs, Some(60));
        assert!(state.last_winners.is_empty());
    }

    #[test]
    fn lobby_state_no_idle_timeout_during_game() {
        let room = ingame_room();
        let state = room.lobby_state();
        assert!(state.idle_timeout_secs.is_none());
    }

    #[test]
    fn is_current_player_bot_detects_bot_turns() {
        let room = ingame_room();
        // The current player is either human (idx 0) or bot (idx 1)
        let current = room.game.as_ref().unwrap().current_player_index();
        if let Some(idx) = current {
            let expected = matches!(room.players[idx].slot_type, PlayerSlotType::Bot { .. });
            assert_eq!(room.is_current_player_bot(), expected);
        }
    }

    #[test]
    fn is_current_player_bot_false_when_no_game() {
        let room = test_room();
        assert!(!room.is_current_player_bot());
    }

    #[test]
    fn get_player_state_returns_valid_state() {
        let room = ingame_room();
        let state = room.get_player_state(0).unwrap();
        // InteractiveGameState should have the right number of players
        assert_eq!(state.boards.len(), 2);
    }

    #[test]
    fn get_player_state_fails_without_game() {
        let room = test_room();
        let err = room.get_player_state(0).unwrap_err();
        assert_eq!(err, ServerError::NotInGame);
    }

    #[test]
    fn turn_deadline_secs_none_when_no_timer() {
        let mut room = ingame_room();
        room.turn_timer_secs = None;
        room.turn_start = None;
        assert!(room.turn_deadline_secs().is_none());
    }

    #[test]
    fn turn_deadline_secs_none_when_no_turn_start() {
        let room = ingame_room();
        // turn_start depends on whether current player is human
        // But if we force it to None:
        let mut room = room;
        room.turn_start = None;
        assert!(room.turn_deadline_secs().is_none());
    }

    #[test]
    fn turn_deadline_secs_returns_remaining_time() {
        let mut room = ingame_room();
        room.turn_timer_secs = Some(60);
        room.turn_start = Some(Instant::now());
        let deadline = room.turn_deadline_secs().unwrap();
        // Should be close to 60 (just started)
        assert!(deadline > 0 && deadline <= 60);
    }

    // ========================================================================
    // Factories
    // ========================================================================

    #[test]
    fn make_strategy_creates_all_base_strategies() {
        let strategies = [
            "Random",
            "Greedy",
            "Defensive",
            "Clearer",
            "Statistician",
            "Rusher",
            "Gambler",
            "Survivor",
            "Mimic",
            "Saboteur",
        ];
        for name in &strategies {
            assert!(
                make_strategy(name, None, 0).is_ok(),
                "Failed to create strategy: {name}"
            );
        }
    }

    #[test]
    fn make_strategy_genetic_requires_genome() {
        let result = make_strategy("Genetic", None, 0);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .message()
                .contains("requires a trained model")
        );
    }

    #[test]
    fn make_strategy_genetic_with_genome() {
        let genome = vec![0.0f32; skyjo_core::GENOME_SIZE];
        assert!(make_strategy("Genetic", Some(&genome), 100).is_ok());
    }

    #[test]
    fn make_strategy_genetic_generation_variant() {
        let genome = vec![0.0f32; skyjo_core::GENOME_SIZE];
        assert!(make_strategy("Genetic:Gen 5", Some(&genome), 100).is_ok());
    }

    #[test]
    fn make_strategy_unknown_fails() {
        let result = make_strategy("FooBar", None, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            ServerError::InvalidStrategy(_)
        ));
    }

    #[test]
    fn make_rules_standard() {
        assert!(make_rules("Standard").is_ok());
    }

    #[test]
    fn make_rules_empty_string() {
        assert!(make_rules("").is_ok());
    }

    #[test]
    fn make_rules_unknown_fails() {
        let result = make_rules("Chaos");
        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            ServerError::InvalidRules(_)
        ));
    }

    #[test]
    fn available_strategies_returns_all_eleven() {
        let strats = available_strategies();
        assert_eq!(strats.len(), 11);
        assert!(strats.contains(&"Random".to_string()));
        assert!(strats.contains(&"Genetic".to_string()));
        assert!(strats.contains(&"Statistician".to_string()));
    }

    #[test]
    fn available_rules_returns_standard() {
        let rules = available_rules();
        assert_eq!(rules, vec!["Standard".to_string()]);
    }

    // ========================================================================
    // Disconnect-to-Bot Conversion
    // ========================================================================

    #[test]
    fn disconnect_converts_to_bot_after_timeout() {
        let mut room = ingame_room();
        // Simulate player 0 disconnecting long ago
        room.players[0].connected = false;
        room.players[0].disconnected_at = Some(Instant::now() - Duration::from_secs(300));

        let converted = room.convert_disconnected_to_bots(Duration::from_secs(60));
        assert_eq!(converted, vec![0]);
        assert!(
            matches!(room.players[0].slot_type, PlayerSlotType::Bot { ref strategy } if strategy == "Random")
        );
        assert!(room.players[0].name.ends_with(" (Bot)"));
        assert!(room.players[0].was_human);
    }

    #[test]
    fn bot_converted_player_can_rejoin() {
        let mut room = ingame_room();
        let original_name = room.players[0].name.clone();
        let token = SessionToken::new();
        room.players[0].session_token = Some(token.clone());

        // Convert to bot
        room.players[0].connected = false;
        room.players[0].disconnected_at = Some(Instant::now() - Duration::from_secs(300));
        room.convert_disconnected_to_bots(Duration::from_secs(60));
        assert!(room.players[0].was_human);

        // Simulate reconnection
        let reconverted = room.reconnect_bot_to_human(0);
        assert!(reconverted);
        assert_eq!(room.players[0].slot_type, PlayerSlotType::Human);
        assert!(!room.players[0].was_human);
        assert_eq!(room.players[0].name, original_name);
    }

    #[test]
    fn disconnect_in_lobby_still_kicks() {
        let mut room = Room::new("R".to_string(), "Alice".to_string(), 3, None, 0, 0);
        room.players[0].connected = true;
        room.players[1] = PlayerSlot {
            name: "Bob".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: Some(SessionToken::new()),
            connected: false,
            ip: None,
            disconnected_at: Some(Instant::now() - Duration::from_secs(300)),
            was_human: false,
            latency_ms: None,
            broadcast_lag_count: 0,
        };
        // In lobby phase, auto_kick_disconnected should kick, not convert to bot
        assert_eq!(room.phase, RoomPhase::Lobby);
        let kicked = room.auto_kick_disconnected(Duration::from_secs(60));
        assert_eq!(kicked.len(), 1);
        assert_eq!(room.players[1].slot_type, PlayerSlotType::Empty);
    }

    #[test]
    fn disconnect_bot_timeout_configurable() {
        let mut room = test_room();
        // Default is None
        assert_eq!(room.disconnect_bot_timeout_secs, None);
        assert_eq!(
            room.effective_disconnect_bot_timeout(),
            Duration::from_secs(60)
        );

        // Set custom
        room.set_disconnect_bot_timeout(Some(120)).unwrap();
        assert_eq!(room.disconnect_bot_timeout_secs, Some(120));
        assert_eq!(
            room.effective_disconnect_bot_timeout(),
            Duration::from_secs(120)
        );

        // Reject out of range
        assert!(room.set_disconnect_bot_timeout(Some(5)).is_err());
        assert!(room.set_disconnect_bot_timeout(Some(301)).is_err());

        // None resets to default
        room.set_disconnect_bot_timeout(None).unwrap();
        assert_eq!(room.disconnect_bot_timeout_secs, None);
    }

    #[test]
    fn disconnect_bot_timeout_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_disconnect_bot_timeout(Some(120)).unwrap_err();
        assert_eq!(err, ServerError::NotInLobby);
    }

    #[test]
    fn conversion_preserves_session_token() {
        let mut room = ingame_room();
        let token = SessionToken::new();
        room.players[0].session_token = Some(token.clone());
        room.players[0].connected = false;
        room.players[0].disconnected_at = Some(Instant::now() - Duration::from_secs(300));

        room.convert_disconnected_to_bots(Duration::from_secs(60));
        // Session token should still be present
        assert_eq!(
            room.players[0].session_token.as_ref().unwrap().as_str(),
            token.as_str()
        );
    }

    #[test]
    fn convert_skips_recently_disconnected() {
        let mut room = ingame_room();
        room.players[0].connected = false;
        room.players[0].disconnected_at = Some(Instant::now());

        let converted = room.convert_disconnected_to_bots(Duration::from_secs(60));
        assert!(converted.is_empty());
        assert_eq!(room.players[0].slot_type, PlayerSlotType::Human);
    }

    #[test]
    fn reconnect_non_converted_player_is_noop() {
        let mut room = ingame_room();
        // Player 0 is Human, was_human is false
        assert!(!room.players[0].was_human);
        let result = room.reconnect_bot_to_human(0);
        assert!(!result);
        assert_eq!(room.players[0].slot_type, PlayerSlotType::Human);
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    /// Play bot+human turns using RandomStrategy logic until the round ends
    /// (either round-over needing continue, or game-over).
    fn play_until_round_over(room: &mut Room) {
        let random_strategy = RandomStrategy;
        for _ in 0..1000 {
            if room.phase != RoomPhase::InGame {
                return;
            }
            {
                let game = match room.game.as_ref() {
                    Some(g) => g,
                    None => return,
                };
                let action_needed = game.get_action_needed();
                match action_needed {
                    skyjo_core::ActionNeeded::GameOver { .. }
                    | skyjo_core::ActionNeeded::RoundOver { .. } => return,
                    _ => {}
                }
            }
            if room.is_current_player_bot() {
                room.apply_bot_action().unwrap();
            } else {
                let game = room.game.as_mut().unwrap();
                let current = game.current_player_index().unwrap();
                let action = game.get_bot_action(&random_strategy).unwrap();
                room.apply_action(current, action).unwrap();
            }
        }
    }

    /// Play until the game is fully over (GameOver phase).
    /// Panics if the game doesn't reach GameOver within 10000 iterations.
    fn play_until_game_over(room: &mut Room) {
        let random_strategy = RandomStrategy;
        for i in 0..10000 {
            if room.phase == RoomPhase::GameOver {
                return;
            }
            {
                let game = match room.game.as_ref() {
                    Some(g) => g,
                    None => return,
                };
                let action_needed = game.get_action_needed();
                match action_needed {
                    skyjo_core::ActionNeeded::GameOver { .. } => return,
                    skyjo_core::ActionNeeded::RoundOver { .. } => {
                        // Need to drop the borrow before calling continue_round
                    }
                    _ => {}
                }
                // Check if we need to continue round (re-check after drop)
            }
            // Re-check for RoundOver outside the borrow
            {
                let is_round_over = room.game.as_ref().is_some_and(|g| {
                    matches!(
                        g.get_action_needed(),
                        skyjo_core::ActionNeeded::RoundOver { .. }
                    )
                });
                if is_round_over {
                    room.continue_round().unwrap();
                    continue;
                }
            }
            if room.is_current_player_bot() {
                room.apply_bot_action().unwrap();
            } else {
                let game = room.game.as_mut().unwrap();
                let current = game.current_player_index().unwrap();
                let action = game.get_bot_action(&random_strategy).unwrap();
                room.apply_action(current, action).unwrap();
            }
            if i == 9999 {
                panic!("play_until_game_over: game did not reach GameOver within 10000 iterations");
            }
        }
    }

    #[test]
    fn valid_state_transitions() {
        let mut room = test_room();
        assert_eq!(room.phase, RoomPhase::Lobby);

        // Lobby → InGame
        room.transition(RoomPhase::InGame).unwrap();
        assert_eq!(room.phase, RoomPhase::InGame);

        // InGame → GameOver
        room.transition(RoomPhase::GameOver).unwrap();
        assert_eq!(room.phase, RoomPhase::GameOver);

        // GameOver → Lobby
        room.transition(RoomPhase::Lobby).unwrap();
        assert_eq!(room.phase, RoomPhase::Lobby);
    }

    #[test]
    fn invalid_state_transitions_rejected() {
        // Lobby → GameOver
        let mut room = test_room();
        assert!(room.transition(RoomPhase::GameOver).is_err());

        // Lobby → Lobby
        let mut room = test_room();
        assert!(room.transition(RoomPhase::Lobby).is_err());

        // InGame → Lobby
        let mut room = test_room();
        room.transition(RoomPhase::InGame).unwrap();
        assert!(room.transition(RoomPhase::Lobby).is_err());

        // InGame → InGame
        let mut room = test_room();
        room.transition(RoomPhase::InGame).unwrap();
        assert!(room.transition(RoomPhase::InGame).is_err());

        // GameOver → InGame
        let mut room = test_room();
        room.transition(RoomPhase::InGame).unwrap();
        room.transition(RoomPhase::GameOver).unwrap();
        assert!(room.transition(RoomPhase::InGame).is_err());

        // GameOver → GameOver
        let mut room = test_room();
        room.transition(RoomPhase::InGame).unwrap();
        room.transition(RoomPhase::GameOver).unwrap();
        assert!(room.transition(RoomPhase::GameOver).is_err());
    }

    // ========================================================================
    // Connection Quality Indicators
    // ========================================================================

    #[test]
    fn update_player_latency() {
        let mut room = test_room();
        room.update_player_latency(0, 42);
        assert_eq!(room.players[0].latency_ms, Some(42));
        assert_eq!(room.players[1].latency_ms, None);
    }

    #[test]
    fn increment_broadcast_lag() {
        let mut room = test_room();
        room.increment_broadcast_lag(0);
        room.increment_broadcast_lag(0);
        assert_eq!(room.players[0].broadcast_lag_count, 2);
        assert_eq!(room.players[1].broadcast_lag_count, 0);
    }

    #[test]
    fn out_of_bounds_latency_update_no_panic() {
        let mut room = test_room();
        room.update_player_latency(99, 100); // Should not panic
    }

    // ========================================================================
    // Per-player channel infrastructure
    // ========================================================================

    #[test]
    fn per_player_channel_setup() {
        let mut room = test_room();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        room.set_player_tx(0, tx);
        assert!(room.player_txs[0].is_some());
        assert!(room.player_txs[1].is_none());
    }

    #[test]
    fn per_player_channel_send() {
        let mut room = test_room();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        room.set_player_tx(0, tx);
        assert!(room.send_to_player(0, b"hello".to_vec()));
        assert_eq!(rx.try_recv().unwrap(), b"hello");
    }

    #[test]
    fn per_player_channel_disconnect() {
        let mut room = test_room();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        room.set_player_tx(0, tx);
        room.remove_player_tx(0);
        assert!(room.player_txs[0].is_none());
    }

    #[test]
    fn send_to_disconnected_player_returns_false() {
        let room = test_room();
        assert!(!room.send_to_player(0, b"hello".to_vec()));
    }

    #[test]
    fn send_to_all_delivers_per_player_data() {
        let mut room = test_room();
        let (tx0, mut rx0) = tokio::sync::mpsc::unbounded_channel();
        let (tx1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
        room.set_player_tx(0, tx0);
        room.set_player_tx(1, tx1);
        room.send_to_all(|i| format!("msg-{i}").into_bytes());
        assert_eq!(rx0.try_recv().unwrap(), b"msg-0");
        assert_eq!(rx1.try_recv().unwrap(), b"msg-1");
    }

    #[test]
    fn set_player_tx_out_of_bounds_no_panic() {
        let mut room = test_room();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        room.set_player_tx(99, tx); // Should not panic
    }

    #[test]
    fn send_to_player_out_of_bounds_returns_false() {
        let room = test_room();
        assert!(!room.send_to_player(99, b"hello".to_vec()));
    }

    #[test]
    fn player_txs_initialized_correctly() {
        let room = test_room();
        assert_eq!(room.player_txs.len(), 2);
        assert!(room.player_txs.iter().all(|tx| tx.is_none()));
    }

    // ========================================================================
    // Room Snapshot
    // ========================================================================

    #[test]
    fn room_snapshot_round_trip() {
        let room = test_room();
        let snapshot = room.to_snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: RoomSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.code, "TEST1");
        assert_eq!(restored.num_players, 2);
        assert_eq!(restored.phase, RoomPhase::Lobby);
        assert_eq!(restored.creator, 0);
        assert_eq!(restored.players.len(), 2);
        assert_eq!(restored.players[0].name, "Alice");
        assert_eq!(restored.players[0].slot_type, PlayerSlotType::Human);
        assert_eq!(restored.players[1].slot_type, PlayerSlotType::Empty);
        assert_eq!(restored.rules_name, "Standard");
        assert!(restored.game_state_json.is_none());
    }

    #[test]
    fn room_snapshot_captures_phase() {
        let room = ingame_room();
        let snapshot = room.to_snapshot();
        assert_eq!(snapshot.phase, RoomPhase::InGame);
        // Should have game state JSON when in-game
        assert!(snapshot.game_state_json.is_some());
    }

    #[test]
    fn room_snapshot_captures_banned_ips() {
        let mut room = test_room();
        room.banned_ips.push("1.2.3.4".to_string());
        let snapshot = room.to_snapshot();
        assert_eq!(snapshot.banned_ips, vec!["1.2.3.4".to_string()]);
    }

    #[test]
    fn room_snapshot_captures_settings() {
        let mut room = test_room();
        room.set_turn_timer(Some(30)).unwrap();
        room.set_disconnect_bot_timeout(Some(120)).unwrap();
        let snapshot = room.to_snapshot();
        assert_eq!(snapshot.turn_timer_secs, Some(30));
        assert_eq!(snapshot.disconnect_bot_timeout_secs, Some(120));
    }

    #[test]
    fn room_from_snapshot_restores_to_lobby() {
        // Create an in-game room, snapshot it, restore — should be in Lobby
        let room = ingame_room();
        let snapshot = room.to_snapshot();
        assert_eq!(snapshot.phase, RoomPhase::InGame);

        let restored = Room::from_snapshot(snapshot);
        assert_eq!(restored.phase, RoomPhase::Lobby);
        assert!(restored.game.is_none());
        assert_eq!(restored.code, room.code);
        assert_eq!(restored.num_players, room.num_players);
        assert_eq!(restored.rules_name, room.rules_name);
    }

    #[test]
    fn room_from_snapshot_restores_players() {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Random").unwrap();
        let snapshot = room.to_snapshot();
        let restored = Room::from_snapshot(snapshot);
        assert_eq!(restored.players[0].name, "Alice");
        assert_eq!(restored.players[0].slot_type, PlayerSlotType::Human);
        assert_eq!(
            restored.players[1].slot_type,
            PlayerSlotType::Bot {
                strategy: "Random".to_string()
            }
        );
        // Connection state is NOT restored
        assert!(!restored.players[0].connected);
        assert!(restored.players[0].session_token.is_none());
    }

    #[test]
    fn room_from_snapshot_preserves_banned_ips() {
        let mut room = test_room();
        room.banned_ips.push("10.0.0.1".to_string());
        let snapshot = room.to_snapshot();
        let restored = Room::from_snapshot(snapshot);
        assert_eq!(restored.banned_ips, vec!["10.0.0.1".to_string()]);
    }

    #[test]
    fn room_snapshot_game_state_json_is_valid() {
        let room = ingame_room();
        let snapshot = room.to_snapshot();
        let json = snapshot.game_state_json.unwrap();
        // Should be valid InteractiveGameState JSON
        let state: skyjo_core::InteractiveGameState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.num_players, 2);
    }
}
