use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use tokio::sync::{broadcast, Mutex};

use skyjo_core::{
    ClearerStrategy, DefensiveStrategy, GamblerStrategy, GeneticStrategy, GreedyStrategy,
    InteractiveGame, InteractiveGameState, MimicStrategy, PlayerAction, RandomStrategy,
    RusherStrategy, Rules, SaboteurStrategy, StandardRules, StatisticianStrategy, Strategy,
    SurvivorStrategy,
};

use crate::messages::{
    LobbyPlayer, PlayerSlotType, RoomLobbyState, ServerMessage,
};
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
    pub fn new(code: String, creator_name: String, num_players: usize, rules: Option<String>, genetic_games_trained: usize, genetic_generation: usize) -> Self {
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
    pub fn configure_slot(
        &mut self,
        slot: usize,
        player_type: &str,
    ) -> Result<(), String> {
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
            let action = game
                .get_bot_action(&strategy)
                .map_err(|e| e.to_string())?;
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
                    let token = self.players[i].session_token.as_ref().map(|t| t.to_string());
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
                let shares_ip = if i != self.creator && matches!(p.slot_type, PlayerSlotType::Human) {
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
                    ServerMessage::GameState { state, turn_deadline_secs: deadline },
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
    ) {
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
    pub fn broadcast_timeout_action(
        &self,
        player: usize,
        action: &PlayerAction,
    ) {
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
            Ok(Box::new(GeneticStrategy::new(genome, genetic_games_trained)))
        }
        s if s.starts_with("Genetic:") => {
            // Specific saved generation: "Genetic:Gen 50"
            // Genome is provided via genetic_genome (resolved by caller)
            let genome = genetic_genome
                .ok_or("Saved genetic generation not found")?
                .clone();
            Ok(Box::new(GeneticStrategy::new(genome, genetic_games_trained)))
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
