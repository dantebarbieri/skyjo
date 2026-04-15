//! Resilience-focused tests.

use std::time::{Duration, Instant};

use skyjo_server::messages::PlayerSlotType;
use skyjo_server::persistence::Persistence;
use skyjo_server::room::{PlayerSlot, Room, RoomPhase};
use skyjo_server::session::SessionToken;

// ========================================================================
// Helpers
// ========================================================================

fn test_room() -> Room {
    Room::new("RESIL1".to_string(), "Alice".to_string(), 2, None, 0, 0)
}

fn filled_room() -> Room {
    let mut room = test_room();
    room.configure_slot(1, "Bot:Random").unwrap();
    room
}

fn ingame_room() -> Room {
    let mut room = filled_room();
    room.start_game().unwrap();
    room
}

// ========================================================================
// Room snapshot round-trip through persistence
// ========================================================================

// TODO: Update this test when room snapshot normalization is completed.
// Currently save/load are stubbed (save is no-op, load returns None) after
// migration 003 replaced the BYTEA snapshot column with relational columns.
#[tokio::test]
async fn snapshot_round_trip_through_persistence() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };
    let db = Persistence::connect(&database_url).await.unwrap();

    let room = test_room();
    let snapshot = room.to_snapshot();
    let json = serde_json::to_string(&snapshot).unwrap();

    // save_room_snapshot is currently a no-op stub — should not error
    db.save_room_snapshot("RESIL1", &json).await.unwrap();

    // load_room_snapshot is currently stubbed to return None
    let loaded = db.load_room_snapshot("RESIL1").await.unwrap();
    assert!(
        loaded.is_none(),
        "load_room_snapshot should return None while stubbed"
    );

    // load_all_room_snapshots is currently stubbed to return empty vec
    let all = db.load_all_room_snapshots().await.unwrap();
    assert!(
        all.is_empty(),
        "load_all_room_snapshots should return empty while stubbed"
    );
}

// ========================================================================
// Persistence connects and creates tables
// ========================================================================

// TODO: Update this test when room snapshot normalization is completed.
// Currently save/load are stubbed after migration 003 replaced the BYTEA column.
#[tokio::test]
async fn persistence_creates_tables_on_connect() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };
    let db = Persistence::connect(&database_url).await.unwrap();

    // save_room_snapshot is currently a no-op stub — should not error
    db.save_room_snapshot("TEST01", r#"{"test":true}"#)
        .await
        .unwrap();

    // load_room_snapshot is currently stubbed to return None
    let loaded = db.load_room_snapshot("TEST01").await.unwrap();
    assert!(
        loaded.is_none(),
        "load_room_snapshot should return None while stubbed"
    );
}

// ========================================================================
// Room conversion to bot preserves game integrity
// ========================================================================

#[test]
fn convert_disconnected_to_bot_preserves_integrity() {
    let mut room = Room::new("BOTCV".to_string(), "Alice".to_string(), 2, None, 0, 0);

    // Slot 1: a human player
    room.players[1].name = "Bob".to_string();
    room.players[1].slot_type = PlayerSlotType::Human;
    room.players[1].session_token = Some(SessionToken::new());
    room.players[1].connected = false;
    room.players[1].disconnected_at = Some(Instant::now() - Duration::from_secs(120));

    // Ready up players before starting
    room.players[1].ready = true;

    // Start game (need all slots filled as human or bot)
    room.start_game().unwrap();
    assert_eq!(room.phase, RoomPhase::InGame);

    // Convert disconnected player to bot
    let converted = room.convert_disconnected_to_bots(Duration::from_secs(60));
    assert_eq!(converted, vec![1]);

    // Verify slot type is Bot
    assert!(matches!(
        room.players[1].slot_type,
        PlayerSlotType::Bot { .. }
    ));
    // Verify name has "(Bot)" suffix
    assert!(room.players[1].name.ends_with(" (Bot)"));
    // Verify was_human flag
    assert!(room.players[1].was_human);
}

// ========================================================================
// Reconnection after bot conversion
// ========================================================================

#[test]
fn reconnect_bot_to_human_restores_player() {
    let mut room = Room::new("RECON".to_string(), "Alice".to_string(), 2, None, 0, 0);

    // Slot 1: a human player
    room.players[1].name = "Charlie".to_string();
    room.players[1].slot_type = PlayerSlotType::Human;
    room.players[1].session_token = Some(SessionToken::new());
    room.players[1].connected = false;
    room.players[1].disconnected_at = Some(Instant::now() - Duration::from_secs(120));

    // Ready up players before starting
    room.players[1].ready = true;

    // Start game
    room.start_game().unwrap();

    // Convert to bot
    let converted = room.convert_disconnected_to_bots(Duration::from_secs(60));
    assert_eq!(converted, vec![1]);
    assert!(room.players[1].was_human);
    assert_eq!(room.players[1].name, "Charlie (Bot)");

    // Reconnect: bot → human
    let result = room.reconnect_bot_to_human(1);
    assert!(result);
    assert_eq!(room.players[1].slot_type, PlayerSlotType::Human);
    assert!(!room.players[1].was_human);
    assert_eq!(room.players[1].name, "Charlie");
}

#[test]
fn reconnect_bot_to_human_returns_false_for_non_converted_bot() {
    let mut room = filled_room(); // slot 1 is a regular Bot:Random
    // was_human is false for a configured bot
    assert!(!room.players[1].was_human);
    let result = room.reconnect_bot_to_human(1);
    assert!(!result);
}

// ========================================================================
// Snapshot captures phase correctly
// ========================================================================

#[test]
fn snapshot_captures_lobby_phase() {
    let room = test_room();
    let snapshot = room.to_snapshot();
    assert_eq!(snapshot.phase, RoomPhase::Lobby);
    assert!(snapshot.game_state_json.is_none());
}

#[test]
fn snapshot_captures_ingame_phase() {
    let room = ingame_room();
    let snapshot = room.to_snapshot();
    assert_eq!(snapshot.phase, RoomPhase::InGame);
    assert!(snapshot.game_state_json.is_some());
}

#[test]
fn from_snapshot_always_restores_to_lobby() {
    let room = ingame_room();
    let snapshot = room.to_snapshot();
    assert_eq!(snapshot.phase, RoomPhase::InGame);

    let restored = Room::from_snapshot(snapshot);
    // from_snapshot always restores to Lobby (game state is not restorable)
    assert_eq!(restored.phase, RoomPhase::Lobby);
    assert!(restored.game.is_none());
}

// ========================================================================
// All players disconnect triggers cleanup eligibility
// ========================================================================

#[test]
fn all_disconnected_room_is_cleanup_eligible() {
    let lobby = skyjo_server::lobby::Lobby::new(10);
    let (code, _token, _) = lobby.create_room("Solo".into(), 2, None, 0, 0).unwrap();

    {
        let room_ref = lobby.rooms.get(&code).unwrap().clone();
        let mut room = room_ref.blocking_lock();
        // Mark all players as disconnected
        for p in room.players.iter_mut() {
            p.connected = false;
        }
        // Set old activity time
        room.last_activity = Instant::now() - Duration::from_secs(600);
    }

    // Cleanup with a short disconnect timeout should remove the room
    lobby.cleanup_stale_rooms(Duration::from_secs(60), Duration::from_secs(60));
    assert!(
        lobby.rooms.is_empty(),
        "disconnected room should be cleaned up"
    );
}

#[test]
fn connected_room_not_cleaned_up() {
    let lobby = skyjo_server::lobby::Lobby::new(10);
    let (code, _token, _) = lobby.create_room("Active".into(), 2, None, 0, 0).unwrap();

    {
        let room_ref = lobby.rooms.get(&code).unwrap().clone();
        let mut room = room_ref.blocking_lock();
        // Creator is connected
        room.players[0].connected = true;
    }

    // Even with aggressive cleanup, connected room should survive
    lobby.cleanup_stale_rooms(Duration::from_secs(60), Duration::from_secs(60));
    assert!(
        lobby.rooms.contains_key(&code),
        "connected room should not be cleaned up"
    );
}

// ========================================================================
// Snapshot preserves banned IPs and settings
// ========================================================================

#[test]
fn snapshot_preserves_banned_ips() {
    let mut room = test_room();
    room.banned_ips.push("10.0.0.99".to_string());
    room.banned_ips.push("172.16.0.1".to_string());

    let snapshot = room.to_snapshot();
    assert_eq!(snapshot.banned_ips.len(), 2);
    assert!(snapshot.banned_ips.contains(&"10.0.0.99".to_string()));
    assert!(snapshot.banned_ips.contains(&"172.16.0.1".to_string()));

    // Restore and verify
    let restored = Room::from_snapshot(snapshot);
    assert!(restored.is_ip_banned("10.0.0.99"));
    assert!(restored.is_ip_banned("172.16.0.1"));
    assert!(!restored.is_ip_banned("8.8.8.8"));
}

#[test]
fn snapshot_preserves_turn_timer_and_disconnect_timeout() {
    let mut room = test_room();
    room.set_turn_timer(Some(30)).unwrap();
    room.set_disconnect_bot_timeout(Some(120)).unwrap();

    let snapshot = room.to_snapshot();
    assert_eq!(snapshot.turn_timer_secs, Some(30));
    assert_eq!(snapshot.disconnect_bot_timeout_secs, Some(120));

    let restored = Room::from_snapshot(snapshot);
    assert_eq!(restored.turn_timer_secs, Some(30));
    assert_eq!(restored.disconnect_bot_timeout_secs, Some(120));
}

// ========================================================================
// Ready state resilience
// ========================================================================

#[test]
fn ready_state_resets_on_snapshot_restore() {
    // Ready state is not persisted in snapshots — restored rooms start with ready=false
    let mut room = test_room();
    room.players[1] = PlayerSlot {
        name: "Bob".to_string(),
        slot_type: PlayerSlotType::Human,
        session_token: Some(SessionToken::new()),
        connected: true,
        ip: None,
        disconnected_at: None,
        was_human: false,
        latency_ms: None,
        broadcast_lag_count: 0,
        user_id: None,
        ready: true,
    };

    let snapshot = room.to_snapshot();
    let restored = Room::from_snapshot(snapshot);
    // Restored players should have ready=false (default)
    assert!(!restored.players[0].ready);
    assert!(!restored.players[1].ready);
    assert!(restored.round_ready.is_empty());
}

#[test]
fn ready_state_preserved_through_bot_conversion_and_rejoin() {
    let mut room = Room::new("READY".to_string(), "Alice".to_string(), 3, None, 0, 0);
    room.players[1] = PlayerSlot {
        name: "Bob".to_string(),
        slot_type: PlayerSlotType::Human,
        session_token: Some(SessionToken::new()),
        connected: true,
        ip: None,
        disconnected_at: None,
        was_human: false,
        latency_ms: None,
        broadcast_lag_count: 0,
        user_id: None,
        ready: true,
    };
    room.players[2] = PlayerSlot {
        name: "Charlie".to_string(),
        slot_type: PlayerSlotType::Human,
        session_token: Some(SessionToken::new()),
        connected: true,
        ip: None,
        disconnected_at: None,
        was_human: false,
        latency_ms: None,
        broadcast_lag_count: 0,
        user_id: None,
        ready: true,
    };

    // Start game (all ready)
    room.start_game().unwrap();
    assert_eq!(room.phase, RoomPhase::InGame);

    // Disconnect player 1 and convert to bot
    room.players[1].connected = false;
    room.players[1].disconnected_at = Some(Instant::now() - Duration::from_secs(300));
    let converted = room.convert_disconnected_to_bots(Duration::from_secs(60));
    assert_eq!(converted, vec![1]);

    // Rejoin: bot → human
    let reconverted = room.reconnect_bot_to_human(1);
    assert!(reconverted);
    assert_eq!(room.players[1].slot_type, PlayerSlotType::Human);
    // ready state is still true (was set before game start)
    assert!(room.players[1].ready);
}
