use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use tokio::sync::{Mutex, broadcast};

use skyjo_core::{
    ClearerStrategy, DefensiveStrategy, GamblerStrategy, GeneticStrategy, GreedyStrategy,
    InteractiveGame, InteractiveGameState, MimicStrategy, PlayerAction, RandomStrategy, Rules,
    RusherStrategy, SaboteurStrategy, StandardRules, StatisticianStrategy, Strategy,
    SurvivorStrategy,
};

use crate::messages::{LobbyPlayer, PlayerSlotType, RoomLobbyState, ServerMessage};
use crate::session::SessionToken;

/// Room lifecycle state.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomPhase {
    Lobby,
    InGame,
    GameOver,
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
            banned_ips: Vec::new(),
            last_winners: Vec::new(),
            turn_timer_secs: Some(60),
            turn_start: None,
            genetic_genome: None,
            genetic_games_trained,
            genetic_generation,
        }
    }

    /// Check if an IP is banned from this room.
    pub fn is_ip_banned(&self, ip: &str) -> bool {
        self.banned_ips.iter().any(|b| b == ip)
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
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
    pub fn configure_slot(&mut self, slot: usize, player_type: &str) -> Result<(), String> {
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot configure slots during game".to_string());
        }

        match player_type {
            "Empty" => {
                // Can't empty the creator slot
                if slot == self.creator {
                    return Err("Cannot remove the creator".to_string());
                }
                self.players[slot] = PlayerSlot {
                    name: String::new(),
                    slot_type: PlayerSlotType::Empty,
                    session_token: None,
                    connected: false,
                    ip: None,
                    disconnected_at: None,
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
                };
            }
            _ => return Err(format!("Unknown player type: {player_type}")),
        }

        Ok(())
    }

    /// Change the rule set (lobby only).
    pub fn set_rules(&mut self, rules: &str) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot change rules during game".to_string());
        }
        // Validate the rules name
        make_rules(rules)?;
        self.rules_name = rules.to_string();
        self.touch();
        Ok(())
    }

    /// Set the turn timer (lobby only, creator only).
    pub fn set_turn_timer(&mut self, secs: Option<u64>) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot change turn timer during game".to_string());
        }
        // Validate: must be None (unlimited) or a positive value
        if let Some(0) = secs {
            return Err("Turn timer must be positive".to_string());
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

    /// Get seconds remaining for the current turn, or None if unlimited / not applicable.
    pub fn turn_deadline_secs(&self) -> Option<u64> {
        let timer = self.turn_timer_secs?;
        let start = self.turn_start?;
        let elapsed = start.elapsed().as_secs();
        Some(timer.saturating_sub(elapsed))
    }

    /// Check if the current human player's turn has timed out.
    /// If so, apply a random action and return the (player, action) pair.
    pub fn check_turn_timeout(&mut self) -> Result<Option<(usize, PlayerAction)>, String> {
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
        let (current, action) = {
            let game = self.game.as_mut().ok_or("No active game")?;
            let current = game.current_player_index().ok_or("No active player")?;
            let strategy = RandomStrategy;
            let action = game.get_bot_action(&strategy).map_err(|e| e.to_string())?;
            game.apply_action(action.clone())
                .map_err(|e| e.to_string())?;
            (current, action)
        };

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok(Some((current, action)))
    }

    /// Change the number of player slots. Can add or remove slots from the end.
    /// Cannot reduce below the number of non-empty slots.
    pub fn set_num_players(&mut self, num_players: usize) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot change player count during game".to_string());
        }
        if !(2..=8).contains(&num_players) {
            return Err("Player count must be 2-8".to_string());
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
                });
            }
        } else if num_players < self.num_players {
            // Remove slots from the end, but only if they're empty
            for i in (num_players..self.num_players).rev() {
                if self.players[i].slot_type != PlayerSlotType::Empty {
                    return Err(format!(
                        "Cannot reduce to {num_players} players: slot {} is occupied",
                        i + 1
                    ));
                }
            }
            self.players.truncate(num_players);
        }

        self.num_players = num_players;
        self.touch();
        Ok(())
    }

    /// Kick a player from the room. Returns their session token so the lobby can clean it up.
    pub fn kick_player(&mut self, slot: usize) -> Result<Option<String>, String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot kick players during game".to_string());
        }
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }
        if slot == self.creator {
            return Err("Cannot kick the room creator".to_string());
        }
        if self.players[slot].slot_type == PlayerSlotType::Empty {
            return Err("Slot is already empty".to_string());
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
        };

        self.touch();
        Ok(token)
    }

    /// Ban a player by slot. Internally bans their IP (never exposed to clients).
    /// Returns error if trying to ban the creator or if IPs match.
    pub fn ban_player(&mut self, slot: usize) -> Result<Option<String>, String> {
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }
        if slot == self.creator {
            return Err("Cannot ban the room creator".to_string());
        }

        let player_ip = self.players[slot].ip.clone();
        let creator_ip = self.players[self.creator].ip.clone();

        // Prevent host from banning their own IP (e.g., same network/localhost)
        if let (Some(p_ip), Some(c_ip)) = (&player_ip, &creator_ip)
            && p_ip == c_ip
        {
            return Err("Cannot ban this player — they share your IP address".to_string());
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
    pub fn start_game(&mut self) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Game already started".to_string());
        }
        if !self.all_slots_filled() {
            return Err("Not all player slots are filled".to_string());
        }

        let rules = make_rules(&self.rules_name)?;
        let player_names: Vec<String> = self.players.iter().map(|p| p.name.clone()).collect();
        let seed: u64 = rand::rng().random();

        let game = InteractiveGame::new(rules, self.num_players, player_names, seed)
            .map_err(|e| e.to_string())?;

        self.game = Some(game);
        self.phase = RoomPhase::InGame;
        self.touch();
        self.reset_turn_start();

        Ok(())
    }

    /// Apply a player action.
    pub fn apply_action(
        &mut self,
        player_index: usize,
        action: PlayerAction,
    ) -> Result<(), String> {
        {
            let game = self.game.as_mut().ok_or("No active game")?;

            // Turn ownership check
            let current = game.current_player_index();
            if let Some(expected) = current {
                if expected != player_index {
                    return Err(format!(
                        "Not your turn: expected player {expected}, got {player_index}"
                    ));
                }
            } else {
                return Err("No player actions expected (round/game over)".to_string());
            }

            game.apply_action(action).map_err(|e| e.to_string())?;
        }

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok(())
    }

    /// Apply a bot action for the current player. Returns (player_index, action).
    pub fn apply_bot_action(&mut self) -> Result<(usize, PlayerAction), String> {
        let (current, action) = {
            let game = self.game.as_mut().ok_or("No active game")?;
            let current = game.current_player_index().ok_or("No active player")?;

            let strategy_name = match &self.players[current].slot_type {
                PlayerSlotType::Bot { strategy } => strategy.clone(),
                _ => return Err("Current player is not a bot".to_string()),
            };

            let strategy = make_strategy(
                &strategy_name,
                self.genetic_genome.as_ref(),
                self.genetic_games_trained,
            )?;
            let action = game
                .get_bot_action(strategy.as_ref())
                .map_err(|e| e.to_string())?;
            game.apply_action(action.clone())
                .map_err(|e| e.to_string())?;
            (current, action)
        };

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok((current, action))
    }

    fn check_game_over(&mut self) {
        if let Some(game) = &self.game {
            let action_needed = game.get_action_needed();
            if let skyjo_core::ActionNeeded::GameOver { ref winners, .. } = action_needed {
                self.phase = RoomPhase::GameOver;
                self.last_winners = winners.clone();
            }
        }
    }

    /// Continue to next round.
    pub fn continue_round(&mut self) -> Result<(), String> {
        let game = self.game.as_mut().ok_or("No active game")?;
        game.apply_action(PlayerAction::ContinueToNextRound)
            .map_err(|e| e.to_string())?;
        self.phase = RoomPhase::InGame;
        self.touch();
        self.reset_turn_start();
        Ok(())
    }

    /// Reset for a new game (play again).
    pub fn play_again(&mut self) -> Result<(), String> {
        if self.phase != RoomPhase::GameOver {
            return Err("Game is not over".to_string());
        }
        self.game = None;
        self.phase = RoomPhase::Lobby;
        self.touch();
        Ok(())
    }

    /// Return to lobby after game ends (preserves last_winners for crown display).
    pub fn return_to_lobby(&mut self) -> Result<(), String> {
        if self.phase != RoomPhase::GameOver {
            return Err("Game is not over".to_string());
        }
        // last_winners is already set by check_game_over
        self.game = None;
        self.phase = RoomPhase::Lobby;
        self.touch();
        Ok(())
    }

    /// Promote another player to host.
    pub fn promote_host(&mut self, slot: usize) -> Result<(), String> {
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }
        if !matches!(self.players[slot].slot_type, PlayerSlotType::Human) {
            return Err("Can only promote human players".to_string());
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
                };
                kicked.push((i, token));
            }
        }
        if !kicked.is_empty() {
            self.touch();
        }
        kicked
    }

    /// Get the per-player game state for a specific player.
    pub fn get_player_state(&self, player_index: usize) -> Result<InteractiveGameState, String> {
        let game = self.game.as_ref().ok_or("No active game")?;
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
            last_winners: self.last_winners.clone(),
            genetic_games_trained: self.genetic_games_trained,
            genetic_generation: self.genetic_generation,
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
    pub fn broadcast_action(&self, player: usize, action: &PlayerAction, is_bot: bool) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };

        let deadline = self.turn_deadline_secs();

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
    pub fn broadcast_timeout_action(&self, player: usize, action: &PlayerAction) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };

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
) -> Result<Box<dyn Strategy>, String> {
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
                .ok_or("Genetic strategy requires a trained model")?
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
                .ok_or("Saved genetic generation not found")?
                .clone();
            Ok(Box::new(GeneticStrategy::new(
                genome,
                genetic_games_trained,
            )))
        }
        _ => Err(format!("Unknown strategy: {name}")),
    }
}

fn make_rules(name: &str) -> Result<Box<dyn Rules>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(StandardRules)),
        _ => Err(format!("Unknown rules: {name}")),
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
        assert_eq!(err, "Cannot remove the creator");
    }

    #[test]
    fn configure_slot_invalid_slot() {
        let mut room = test_room();
        let err = room.configure_slot(5, "Empty").unwrap_err();
        assert_eq!(err, "Invalid slot");
    }

    #[test]
    fn configure_slot_unknown_type() {
        let mut room = test_room();
        let err = room.configure_slot(1, "Alien").unwrap_err();
        assert!(err.starts_with("Unknown player type"));
    }

    #[test]
    fn configure_slot_invalid_strategy() {
        let mut room = test_room();
        let err = room.configure_slot(1, "Bot:NonExistent").unwrap_err();
        assert!(err.contains("Unknown strategy"));
    }

    #[test]
    fn configure_slot_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.configure_slot(1, "Bot:Greedy").unwrap_err();
        assert_eq!(err, "Cannot configure slots during game");
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
        assert!(err.contains("slot 3 is occupied"));
    }

    #[test]
    fn set_num_players_invalid_below_2() {
        let mut room = test_room();
        let err = room.set_num_players(1).unwrap_err();
        assert_eq!(err, "Player count must be 2-8");
    }

    #[test]
    fn set_num_players_invalid_above_8() {
        let mut room = test_room();
        let err = room.set_num_players(9).unwrap_err();
        assert_eq!(err, "Player count must be 2-8");
    }

    #[test]
    fn set_num_players_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_num_players(3).unwrap_err();
        assert_eq!(err, "Cannot change player count during game");
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
        assert!(err.contains("Unknown rules"));
    }

    #[test]
    fn set_rules_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_rules("Standard").unwrap_err();
        assert_eq!(err, "Cannot change rules during game");
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
        assert_eq!(err, "Turn timer must be positive");
    }

    #[test]
    fn set_turn_timer_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.set_turn_timer(Some(30)).unwrap_err();
        assert_eq!(err, "Cannot change turn timer during game");
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
        assert_eq!(err, "Not all player slots are filled");
    }

    #[test]
    fn start_game_fails_if_already_started() {
        let mut room = ingame_room();
        let err = room.start_game().unwrap_err();
        assert_eq!(err, "Game already started");
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
        assert!(err.contains("Not your turn"));
    }

    #[test]
    fn apply_bot_action_returns_valid_action() {
        let mut room = ingame_room();
        // If current player isn't a bot, configure so slot 1 is bot and wait for its turn
        if room.is_current_player_bot() {
            let (player_idx, _action) = room.apply_bot_action().unwrap();
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
        assert_eq!(err, "Current player is not a bot");
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
        assert_eq!(err, "Game is not over");
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
        assert_eq!(err, "Game is not over");
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
        assert_eq!(err, "Cannot kick the room creator");
    }

    #[test]
    fn kick_player_rejects_empty_slot() {
        let mut room = test_room();
        let err = room.kick_player(1).unwrap_err();
        assert_eq!(err, "Slot is already empty");
    }

    #[test]
    fn kick_player_rejects_invalid_slot() {
        let mut room = test_room();
        let err = room.kick_player(10).unwrap_err();
        assert_eq!(err, "Invalid slot");
    }

    #[test]
    fn kick_player_rejects_during_game() {
        let mut room = ingame_room();
        let err = room.kick_player(1).unwrap_err();
        assert_eq!(err, "Cannot kick players during game");
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
        };

        let err = room.ban_player(1).unwrap_err();
        assert!(err.contains("share your IP"));
    }

    #[test]
    fn ban_player_rejects_creator() {
        let mut room = test_room();
        let err = room.ban_player(0).unwrap_err();
        assert_eq!(err, "Cannot ban the room creator");
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
        };
        room.promote_host(1).unwrap();
        assert_eq!(room.creator, 1);
    }

    #[test]
    fn promote_host_rejects_bot() {
        let mut room = test_room();
        room.configure_slot(1, "Bot:Random").unwrap();
        let err = room.promote_host(1).unwrap_err();
        assert_eq!(err, "Can only promote human players");
    }

    #[test]
    fn promote_host_rejects_invalid_slot() {
        let mut room = test_room();
        let err = room.promote_host(10).unwrap_err();
        assert_eq!(err, "Invalid slot");
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
        };
        room.players[2] = PlayerSlot {
            name: "Charlie".to_string(),
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: true,
            ip: None,
            disconnected_at: None,
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
        assert_eq!(err, "No active game");
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
        assert!(result.err().unwrap().contains("requires a trained model"));
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
        assert!(result.err().unwrap().contains("Unknown strategy"));
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
        assert!(result.err().unwrap().contains("Unknown rules"));
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
}
