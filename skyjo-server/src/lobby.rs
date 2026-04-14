use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::room::{Room, SharedRoom};
use crate::session::SessionToken;

/// Global lobby managing all rooms.
pub struct Lobby {
    /// Rooms indexed by room code.
    pub rooms: DashMap<String, SharedRoom>,
    /// Session token -> (room_code, player_index).
    pub sessions: DashMap<String, (String, usize)>,
    /// Maximum concurrent rooms.
    pub max_rooms: usize,
}

impl Lobby {
    pub fn new(max_rooms: usize) -> Self {
        Self {
            rooms: DashMap::new(),
            sessions: DashMap::new(),
            max_rooms,
        }
    }

    /// Generate a unique 6-character room code.
    fn generate_code(&self) -> String {
        let chars: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut rng = rand::rng();
        loop {
            let code: String = (0..6)
                .map(|_| chars[rng.random_range(0..chars.len())] as char)
                .collect();
            if !self.rooms.contains_key(&code) {
                return code;
            }
        }
    }

    /// Create a new room. Returns (room_code, session_token, player_index).
    pub fn create_room(
        &self,
        player_name: String,
        num_players: usize,
        rules: Option<String>,
        genetic_games_trained: usize,
        genetic_generation: usize,
    ) -> Result<(String, SessionToken, usize), String> {
        if !(2..=8).contains(&num_players) {
            return Err("Player count must be 2-8".to_string());
        }
        if self.rooms.len() >= self.max_rooms {
            return Err("Server is at maximum room capacity".to_string());
        }

        let code = self.generate_code();
        let token = SessionToken::new();
        let player_index = 0;

        let mut room = Room::new(
            code.clone(),
            player_name,
            num_players,
            rules,
            genetic_games_trained,
            genetic_generation,
        );
        room.players[player_index].session_token = Some(token.clone());

        let shared = Arc::new(Mutex::new(room));
        self.rooms.insert(code.clone(), shared);
        self.sessions
            .insert(token.to_string(), (code.clone(), player_index));

        Ok((code, token, player_index))
    }

    /// Join an existing room. Returns (session_token, player_index).
    pub async fn join_room(
        &self,
        code: &str,
        player_name: String,
    ) -> Result<(SessionToken, usize), String> {
        let room_ref = self.rooms.get(code).ok_or("Room not found")?.clone();

        let mut room = room_ref.lock().await;

        if room.phase != crate::room::RoomPhase::Lobby {
            return Err("Game already started".to_string());
        }

        let slot = room
            .next_available_slot()
            .ok_or("Room is full (all slots are taken by human players)")?;

        let token = SessionToken::new();
        room.players[slot].name = player_name.clone();
        room.players[slot].slot_type = crate::messages::PlayerSlotType::Human;
        room.players[slot].session_token = Some(token.clone());
        room.touch();

        self.sessions
            .insert(token.to_string(), (code.to_string(), slot));

        // Notify other players
        for (i, p) in room.players.iter().enumerate() {
            if i != slot && p.connected {
                let _ = room.broadcast_tx.send((
                    i,
                    crate::messages::ServerMessage::PlayerJoined {
                        player_index: slot,
                        name: player_name.clone(),
                    },
                ));
            }
        }

        Ok((token, slot))
    }

    /// Look up a session token to find the room and player index.
    pub fn get_session(&self, token: &str) -> Option<(String, usize)> {
        self.sessions.get(token).map(|entry| entry.clone())
    }

    /// Get a room by code.
    pub fn get_room(&self, code: &str) -> Option<SharedRoom> {
        self.rooms.get(code).map(|entry| entry.clone())
    }

    /// Clean up inactive rooms.
    pub fn cleanup_stale_rooms(&self, game_over_timeout: Duration, disconnect_timeout: Duration) {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        // We can't await inside the iteration, so collect codes to check
        // For simplicity, use try_lock to avoid blocking
        for entry in self.rooms.iter() {
            let code = entry.key().clone();
            if let Ok(room) = entry.value().try_lock() {
                let elapsed = now.duration_since(room.last_activity);
                let all_disconnected = room.players.iter().all(|p| !p.connected);
                let lobby_idle_timeout = Duration::from_secs(30 * 60); // 30 min
                let should_remove = match room.phase {
                    crate::room::RoomPhase::GameOver => elapsed > game_over_timeout,
                    crate::room::RoomPhase::Lobby => {
                        (all_disconnected && elapsed > disconnect_timeout)
                            || elapsed > lobby_idle_timeout
                    }
                    crate::room::RoomPhase::InGame => {
                        all_disconnected && elapsed > disconnect_timeout
                    }
                };
                if should_remove {
                    // Collect session tokens to remove
                    for player in &room.players {
                        if let Some(token) = &player.session_token {
                            self.sessions.remove(token.as_str());
                        }
                    }
                    to_remove.push(code);
                }
            }
        }

        for code in to_remove {
            self.rooms.remove(&code);
            tracing::info!("Cleaned up stale room: {code}");
        }
    }
}

/// Request body for creating a room.
#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    pub player_name: String,
    pub num_players: usize,
    pub rules: Option<String>,
}

/// Response after creating a room.
#[derive(Debug, Serialize)]
pub struct CreateRoomResponse {
    pub room_code: String,
    pub session_token: String,
    pub player_index: usize,
}

/// Request body for joining a room.
#[derive(Debug, Deserialize)]
pub struct JoinRoomRequest {
    pub player_name: String,
}

/// Response after joining a room.
#[derive(Debug, Serialize)]
pub struct JoinRoomResponse {
    pub session_token: String,
    pub player_index: usize,
}

/// Response for room info (public, no auth needed).
#[derive(Debug, Serialize)]
pub struct RoomInfoResponse {
    pub room_code: String,
    pub num_players: usize,
    pub rules: String,
    pub players_joined: usize,
    pub phase: String,
}
