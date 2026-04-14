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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_lobby(max_rooms: usize) -> Lobby {
        Lobby::new(max_rooms)
    }

    fn create_default_room(lobby: &Lobby) -> (String, SessionToken, usize) {
        lobby
            .create_room("Alice".into(), 2, None, 0, 0)
            .expect("create_room should succeed")
    }

    #[test]
    fn new_lobby_has_correct_max_rooms() {
        let lobby = make_lobby(10);
        assert_eq!(lobby.max_rooms, 10);
        assert!(lobby.rooms.is_empty());
        assert!(lobby.sessions.is_empty());
    }

    #[test]
    fn create_room_returns_valid_code_and_token() {
        let lobby = make_lobby(5);
        let (code, token, player_index) = create_default_room(&lobby);

        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(!token.as_str().is_empty());
        assert_eq!(player_index, 0);
    }

    #[test]
    fn create_room_stores_room_and_session() {
        let lobby = make_lobby(5);
        let (code, token, _) = create_default_room(&lobby);

        assert_eq!(lobby.rooms.len(), 1);
        assert!(lobby.rooms.contains_key(&code));
        assert_eq!(lobby.sessions.len(), 1);
        assert!(lobby.sessions.contains_key(token.as_str()));
    }

    #[test]
    fn create_room_rejects_invalid_player_count() {
        let lobby = make_lobby(5);
        assert!(lobby.create_room("A".into(), 1, None, 0, 0).is_err());
        assert!(lobby.create_room("A".into(), 9, None, 0, 0).is_err());
        assert!(lobby.create_room("A".into(), 0, None, 0, 0).is_err());
    }

    #[test]
    fn max_rooms_enforcement() {
        let lobby = make_lobby(2);
        create_default_room(&lobby);
        create_default_room(&lobby);

        let result = lobby.create_room("Charlie".into(), 2, None, 0, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Server is at maximum room capacity");
    }

    #[test]
    fn room_code_uniqueness() {
        let lobby = make_lobby(50);
        let mut codes = HashSet::new();
        for _ in 0..20 {
            let (code, _, _) = create_default_room(&lobby);
            assert!(codes.insert(code), "duplicate room code generated");
        }
    }

    #[test]
    fn session_lookup_returns_correct_data() {
        let lobby = make_lobby(5);
        let (code, token, player_index) = create_default_room(&lobby);

        let session = lobby.get_session(token.as_str());
        assert!(session.is_some());
        let (sess_code, sess_idx) = session.unwrap();
        assert_eq!(sess_code, code);
        assert_eq!(sess_idx, player_index);
    }

    #[test]
    fn session_lookup_returns_none_for_invalid_token() {
        let lobby = make_lobby(5);
        assert!(lobby.get_session("nonexistent-token").is_none());
    }

    #[test]
    fn get_room_returns_shared_room() {
        let lobby = make_lobby(5);
        let (code, _, _) = create_default_room(&lobby);

        assert!(lobby.get_room(&code).is_some());
    }

    #[test]
    fn get_room_returns_none_for_invalid_code() {
        let lobby = make_lobby(5);
        assert!(lobby.get_room("ZZZZZZ").is_none());
    }

    #[tokio::test]
    async fn join_room_succeeds_with_valid_code() {
        let lobby = make_lobby(5);
        let (code, _, _) = create_default_room(&lobby);

        let result = lobby.join_room(&code, "Bob".into()).await;
        assert!(result.is_ok());
        let (token, player_index) = result.unwrap();
        assert!(!token.as_str().is_empty());
        assert_eq!(player_index, 1);

        // Session should be stored
        let session = lobby.get_session(token.as_str());
        assert!(session.is_some());
        let (sess_code, sess_idx) = session.unwrap();
        assert_eq!(sess_code, code);
        assert_eq!(sess_idx, 1);
    }

    #[tokio::test]
    async fn join_room_fails_with_invalid_code() {
        let lobby = make_lobby(5);
        let result = lobby.join_room("BADCODE", "Bob".into()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Room not found");
    }

    #[tokio::test]
    async fn join_room_fills_slots_in_order() {
        let lobby = make_lobby(5);
        let (code, _, _) = lobby
            .create_room("Alice".into(), 4, None, 0, 0)
            .unwrap();

        let (_, idx1) = lobby.join_room(&code, "Bob".into()).await.unwrap();
        let (_, idx2) = lobby.join_room(&code, "Carol".into()).await.unwrap();
        let (_, idx3) = lobby.join_room(&code, "Dave".into()).await.unwrap();

        assert_eq!(idx1, 1);
        assert_eq!(idx2, 2);
        assert_eq!(idx3, 3);

        // Room is now full — next join should fail
        let result = lobby.join_room(&code, "Eve".into()).await;
        assert!(result.is_err());
    }

    #[test]
    fn cleanup_stale_rooms_removes_old_game_over_rooms() {
        let lobby = make_lobby(5);
        let (code, token, _) = create_default_room(&lobby);

        // Manually set the room to GameOver with old last_activity
        {
            let room_ref = lobby.rooms.get(&code).unwrap().clone();
            let mut room = room_ref.blocking_lock();
            room.phase = crate::room::RoomPhase::GameOver;
            room.last_activity = Instant::now() - Duration::from_secs(3600);
        }

        lobby.cleanup_stale_rooms(Duration::from_secs(60), Duration::from_secs(60));

        assert!(lobby.rooms.is_empty());
        assert!(lobby.get_session(token.as_str()).is_none());
    }

    #[test]
    fn cleanup_does_not_remove_active_rooms() {
        let lobby = make_lobby(5);
        let (code, _, _) = create_default_room(&lobby);

        lobby.cleanup_stale_rooms(Duration::from_secs(3600), Duration::from_secs(3600));

        assert!(lobby.rooms.contains_key(&code));
    }

    #[test]
    fn generate_code_uses_valid_charset() {
        let lobby = make_lobby(5);
        let valid: HashSet<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
        for _ in 0..50 {
            let code = lobby.generate_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| valid.contains(&c)));
        }
    }
}
