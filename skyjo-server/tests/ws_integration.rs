//! WebSocket protocol integration tests.
//! Tests message serialization, protocol compatibility, and wire format.

use skyjo_core::interactive::PlayerAction;
use skyjo_server::messages::*;

// ---------------------------------------------------------------------------
// ClientMessage JSON round-trips
// ---------------------------------------------------------------------------

#[test]
fn all_client_messages_json_round_trip() {
    let messages: Vec<ClientMessage> = vec![
        ClientMessage::Ping,
        ClientMessage::StartGame,
        ClientMessage::ReturnToLobby,
        ClientMessage::PlayAgain,
        ClientMessage::ContinueRound,
        ClientMessage::RequestFullState,
        ClientMessage::SetNumPlayers { num_players: 4 },
        ClientMessage::SetRules {
            rules: "Standard".to_string(),
        },
        ClientMessage::SetTurnTimer { secs: Some(30) },
        ClientMessage::SetTurnTimer { secs: None },
        ClientMessage::SetDisconnectTimeout { secs: Some(120) },
        ClientMessage::SetDisconnectTimeout { secs: None },
        ClientMessage::ConfigureSlot {
            slot: 1,
            player_type: "Bot".to_string(),
        },
        ClientMessage::KickPlayer { slot: 2 },
        ClientMessage::BanPlayer { slot: 3 },
        ClientMessage::PromoteHost { slot: 1 },
        ClientMessage::Action {
            action: PlayerAction::InitialFlip { position: 0 },
        },
        ClientMessage::Action {
            action: PlayerAction::DrawFromDeck,
        },
        ClientMessage::Action {
            action: PlayerAction::DrawFromDiscard { pile_index: 0 },
        },
        ClientMessage::Action {
            action: PlayerAction::UndoDrawFromDiscard,
        },
        ClientMessage::Action {
            action: PlayerAction::KeepDeckDraw { position: 5 },
        },
        ClientMessage::Action {
            action: PlayerAction::DiscardAndFlip { position: 3 },
        },
        ClientMessage::Action {
            action: PlayerAction::PlaceDiscardDraw { position: 7 },
        },
        ClientMessage::Action {
            action: PlayerAction::ContinueToNextRound,
        },
    ];

    for msg in &messages {
        let json = serde_json::to_string(msg).unwrap();
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();
        // Verify round-trip produces valid JSON that deserializes without panic
        let _ = format!("{decoded:?}");
    }
}

// ---------------------------------------------------------------------------
// ClientMessage MessagePack round-trips
// ---------------------------------------------------------------------------

#[test]
fn all_client_messages_msgpack_round_trip() {
    let messages: Vec<ClientMessage> = vec![
        ClientMessage::Ping,
        ClientMessage::StartGame,
        ClientMessage::ReturnToLobby,
        ClientMessage::PlayAgain,
        ClientMessage::ContinueRound,
        ClientMessage::RequestFullState,
        ClientMessage::SetNumPlayers { num_players: 4 },
        ClientMessage::SetRules {
            rules: "Standard".to_string(),
        },
        ClientMessage::SetTurnTimer { secs: Some(30) },
        ClientMessage::SetTurnTimer { secs: None },
        ClientMessage::SetDisconnectTimeout { secs: Some(120) },
        ClientMessage::SetDisconnectTimeout { secs: None },
        ClientMessage::ConfigureSlot {
            slot: 1,
            player_type: "Bot".to_string(),
        },
        ClientMessage::KickPlayer { slot: 2 },
        ClientMessage::BanPlayer { slot: 3 },
        ClientMessage::PromoteHost { slot: 1 },
        ClientMessage::Action {
            action: PlayerAction::DrawFromDeck,
        },
        ClientMessage::Action {
            action: PlayerAction::InitialFlip { position: 0 },
        },
    ];

    for msg in &messages {
        let packed = rmp_serde::to_vec(msg).unwrap();
        let decoded: ClientMessage = rmp_serde::from_slice(&packed).unwrap();
        let _ = format!("{decoded:?}");
    }
}

// ---------------------------------------------------------------------------
// ServerMessage JSON round-trips
// ---------------------------------------------------------------------------

#[test]
fn all_server_messages_json_round_trip() {
    let messages: Vec<ServerMessage> = vec![
        ServerMessage::Pong,
        ServerMessage::ServerShutdown,
        ServerMessage::Error {
            code: "TestError".into(),
            message: "something went wrong".into(),
        },
        ServerMessage::PlayerJoined {
            player_index: 0,
            name: "Alice".into(),
        },
        ServerMessage::PlayerLeft { player_index: 1 },
        ServerMessage::PlayerReconnected { player_index: 2 },
        ServerMessage::PlayerConvertedToBot {
            slot: 0,
            name: "DisconnectedPlayer".into(),
        },
        ServerMessage::Kicked {
            reason: "You were kicked by the room host".into(),
        },
    ];

    for msg in &messages {
        let json = serde_json::to_string(msg).unwrap();
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        let _ = format!("{decoded:?}");
    }
}

// ---------------------------------------------------------------------------
// ServerMessage MessagePack round-trips
// ---------------------------------------------------------------------------

#[test]
fn all_server_messages_msgpack_round_trip() {
    let messages: Vec<ServerMessage> = vec![
        ServerMessage::Pong,
        ServerMessage::ServerShutdown,
        ServerMessage::Error {
            code: "TestError".into(),
            message: "something went wrong".into(),
        },
        ServerMessage::PlayerJoined {
            player_index: 0,
            name: "Alice".into(),
        },
        ServerMessage::PlayerLeft { player_index: 1 },
        ServerMessage::PlayerReconnected { player_index: 2 },
        ServerMessage::PlayerConvertedToBot {
            slot: 0,
            name: "DisconnectedPlayer".into(),
        },
        ServerMessage::Kicked {
            reason: "You were kicked by the room host".into(),
        },
    ];

    for msg in &messages {
        let packed = rmp_serde::to_vec(msg).unwrap();
        let decoded: ServerMessage = rmp_serde::from_slice(&packed).unwrap();
        let _ = format!("{decoded:?}");
    }
}

// ---------------------------------------------------------------------------
// WireFormat: ServerMessage::to_bytes / ClientMessage::from_bytes
// ---------------------------------------------------------------------------

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

#[test]
fn client_message_format_detection_json() {
    let msg = ClientMessage::Ping;
    let json = serde_json::to_string(&msg).unwrap();
    let decoded = ClientMessage::from_bytes(json.as_bytes(), false).unwrap();
    assert!(matches!(decoded, ClientMessage::Ping));
}

#[test]
fn client_message_format_detection_msgpack() {
    let msg = ClientMessage::Ping;
    let msgpack = rmp_serde::to_vec(&msg).unwrap();
    let decoded = ClientMessage::from_bytes(&msgpack, true).unwrap();
    assert!(matches!(decoded, ClientMessage::Ping));
}

#[test]
fn client_message_with_fields_json_round_trip_via_from_bytes() {
    let msg = ClientMessage::SetNumPlayers { num_players: 5 };
    let json = serde_json::to_vec(&msg).unwrap();
    let decoded = ClientMessage::from_bytes(&json, false).unwrap();
    assert!(matches!(
        decoded,
        ClientMessage::SetNumPlayers { num_players: 5 }
    ));
}

#[test]
fn client_message_with_fields_msgpack_round_trip_via_from_bytes() {
    let msg = ClientMessage::SetNumPlayers { num_players: 5 };
    let packed = rmp_serde::to_vec(&msg).unwrap();
    let decoded = ClientMessage::from_bytes(&packed, true).unwrap();
    assert!(matches!(
        decoded,
        ClientMessage::SetNumPlayers { num_players: 5 }
    ));
}

// ---------------------------------------------------------------------------
// Error handling: invalid payloads
// ---------------------------------------------------------------------------

#[test]
fn invalid_json_rejected() {
    let result = ClientMessage::from_bytes(b"not json at all", false);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("JSON"));
}

#[test]
fn invalid_msgpack_rejected() {
    let result = ClientMessage::from_bytes(b"\xff\xff\xff", true);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("MessagePack"));
}

#[test]
fn empty_payload_rejected_json() {
    let result = ClientMessage::from_bytes(b"", false);
    assert!(result.is_err());
}

#[test]
fn empty_payload_rejected_msgpack() {
    let result = ClientMessage::from_bytes(b"", true);
    assert!(result.is_err());
}

#[test]
fn unknown_fields_rejected_json() {
    // ClientMessage uses deny_unknown_fields — extra fields alongside type tag are rejected.
    // Note: for unit variants like Ping, serde may tolerate extra top-level keys depending
    // on the tag representation. Test with a variant that has fields to be rigorous.
    let result = ClientMessage::from_bytes(
        br#"{"type":"SetNumPlayers","num_players":4,"extra_field":true}"#,
        false,
    );
    assert!(result.is_err());
}

#[test]
fn unknown_type_tag_rejected_json() {
    let result = ClientMessage::from_bytes(br#"{"type":"NonExistent"}"#, false);
    assert!(result.is_err());
}

#[test]
fn missing_required_fields_rejected_json() {
    // SetNumPlayers requires num_players
    let result = ClientMessage::from_bytes(br#"{"type":"SetNumPlayers"}"#, false);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// SlotUpdate serde
// ---------------------------------------------------------------------------

#[test]
fn slot_update_variants_json_round_trip() {
    let variants = vec![
        SlotUpdate::Hidden,
        SlotUpdate::Revealed(5),
        SlotUpdate::Revealed(-2),
        SlotUpdate::Revealed(12),
        SlotUpdate::Cleared,
    ];
    for v in &variants {
        let json = serde_json::to_string(v).unwrap();
        let decoded: SlotUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(*v, decoded);
    }
}

#[test]
fn slot_update_variants_msgpack_round_trip() {
    let variants = vec![
        SlotUpdate::Hidden,
        SlotUpdate::Revealed(5),
        SlotUpdate::Revealed(-2),
        SlotUpdate::Revealed(12),
        SlotUpdate::Cleared,
    ];
    for v in &variants {
        let packed = rmp_serde::to_vec(v).unwrap();
        let decoded: SlotUpdate = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(*v, decoded);
    }
}

// ---------------------------------------------------------------------------
// StateDelta serde
// ---------------------------------------------------------------------------

fn sample_delta() -> StateDelta {
    StateDelta {
        board_changes: vec![(0, 3, SlotUpdate::Revealed(5))],
        discard_tops_changed: vec![(0, Some(7))],
        deck_remaining: 80,
        current_player: 1,
        column_clears: vec![],
        action_needed: "WaitingForDraw".to_string(),
        turn_deadline_secs: Some(30.0),
        is_final_turn: false,
        going_out_player: None,
    }
}

#[test]
fn state_delta_json_round_trip() {
    let delta = sample_delta();
    let json = serde_json::to_string(&delta).unwrap();
    let decoded: StateDelta = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.deck_remaining, 80);
    assert_eq!(decoded.current_player, 1);
    assert_eq!(decoded.board_changes.len(), 1);
    assert_eq!(decoded.board_changes[0], (0, 3, SlotUpdate::Revealed(5)));
    assert_eq!(decoded.discard_tops_changed, vec![(0, Some(7))]);
    assert!(!decoded.is_final_turn);
}

#[test]
fn state_delta_msgpack_round_trip() {
    let delta = sample_delta();
    // Use rmp_serde::to_vec_named so field names are preserved (required for serde structs)
    let packed = rmp_serde::to_vec_named(&delta).unwrap();
    let decoded: StateDelta = rmp_serde::from_slice(&packed).unwrap();
    assert_eq!(decoded.deck_remaining, 80);
    assert_eq!(decoded.current_player, 1);
    assert_eq!(decoded.board_changes.len(), 1);
}

#[test]
fn state_delta_optional_fields_omitted_when_empty() {
    let delta = StateDelta {
        board_changes: vec![],
        discard_tops_changed: vec![],
        deck_remaining: 100,
        current_player: 0,
        column_clears: vec![],
        action_needed: "ChooseDraw".to_string(),
        turn_deadline_secs: None,
        is_final_turn: false,
        going_out_player: None,
    };
    let json = serde_json::to_string(&delta).unwrap();
    // Fields with skip_serializing_if should be absent
    assert!(!json.contains("discard_tops_changed"));
    assert!(!json.contains("column_clears"));
    assert!(!json.contains("turn_deadline_secs"));
    assert!(!json.contains("going_out_player"));
}

// ---------------------------------------------------------------------------
// PlayerSlotType serde (integration-level, complements unit tests)
// ---------------------------------------------------------------------------

#[test]
fn player_slot_type_all_variants_json() {
    let variants = vec![
        PlayerSlotType::Human,
        PlayerSlotType::Bot {
            strategy: "SmartBot".to_string(),
        },
        PlayerSlotType::Empty,
    ];
    for v in &variants {
        let json = serde_json::to_string(v).unwrap();
        let decoded: PlayerSlotType = serde_json::from_str(&json).unwrap();
        assert_eq!(*v, decoded);
    }
}

#[test]
fn player_slot_type_all_variants_msgpack() {
    let variants = vec![
        PlayerSlotType::Human,
        PlayerSlotType::Bot {
            strategy: "SmartBot".to_string(),
        },
        PlayerSlotType::Empty,
    ];
    for v in &variants {
        let packed = rmp_serde::to_vec(v).unwrap();
        let decoded: PlayerSlotType = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(*v, decoded);
    }
}

// ---------------------------------------------------------------------------
// Performance: MessagePack vs JSON size comparison
// ---------------------------------------------------------------------------

#[test]
fn msgpack_consistently_smaller_or_equal() {
    let messages: Vec<ServerMessage> = vec![
        ServerMessage::Pong,
        ServerMessage::ServerShutdown,
        ServerMessage::Error {
            code: "RoomNotFound".into(),
            message: "Room not found".into(),
        },
        ServerMessage::PlayerJoined {
            player_index: 0,
            name: "LongPlayerName123".into(),
        },
        ServerMessage::PlayerLeft { player_index: 3 },
        ServerMessage::PlayerReconnected { player_index: 1 },
        ServerMessage::PlayerConvertedToBot {
            slot: 2,
            name: "Disconnected_Player_With_A_Long_Name".into(),
        },
        ServerMessage::Kicked {
            reason: "You were kicked by the room host for inactivity".into(),
        },
    ];

    for msg in &messages {
        let json_size = serde_json::to_vec(msg).unwrap().len();
        let msgpack_size = rmp_serde::to_vec(msg).unwrap().len();
        println!(
            "{:?}: JSON={json_size}B, MessagePack={msgpack_size}B, savings={:.0}%",
            std::mem::discriminant(msg),
            (1.0 - msgpack_size as f64 / json_size as f64) * 100.0
        );
        assert!(
            msgpack_size <= json_size,
            "MessagePack should not be larger than JSON for {:?}",
            std::mem::discriminant(msg)
        );
    }
}

#[test]
fn delta_smaller_than_500_bytes() {
    let delta = StateDelta {
        board_changes: vec![(0, 3, SlotUpdate::Revealed(5))],
        discard_tops_changed: vec![(0, Some(7))],
        deck_remaining: 80,
        current_player: 1,
        column_clears: vec![],
        action_needed: "WaitingForDraw".to_string(),
        turn_deadline_secs: Some(30.0),
        is_final_turn: false,
        going_out_player: None,
    };

    let delta_json = serde_json::to_vec(&delta).unwrap();
    let delta_msgpack = rmp_serde::to_vec(&delta).unwrap();

    println!(
        "Delta: JSON={} bytes, MsgPack={} bytes",
        delta_json.len(),
        delta_msgpack.len()
    );

    // Delta should be well under 500 bytes in either format
    assert!(
        delta_msgpack.len() < 500,
        "Delta MsgPack should be compact, got {} bytes",
        delta_msgpack.len()
    );
    assert!(
        delta_json.len() < 500,
        "Delta JSON should be compact, got {} bytes",
        delta_json.len()
    );
    // MsgPack should be smaller
    assert!(delta_msgpack.len() <= delta_json.len());
}

#[test]
fn client_message_msgpack_smaller_than_json() {
    let messages: Vec<ClientMessage> = vec![
        ClientMessage::Ping,
        ClientMessage::SetNumPlayers { num_players: 4 },
        ClientMessage::SetRules {
            rules: "AuntJanetRules".to_string(),
        },
        ClientMessage::ConfigureSlot {
            slot: 1,
            player_type: "Bot".to_string(),
        },
        ClientMessage::Action {
            action: PlayerAction::KeepDeckDraw { position: 5 },
        },
    ];

    for msg in &messages {
        let json_size = serde_json::to_vec(msg).unwrap().len();
        let msgpack_size = rmp_serde::to_vec(msg).unwrap().len();
        println!(
            "{:?}: JSON={json_size}B, MessagePack={msgpack_size}B",
            std::mem::discriminant(msg),
        );
        assert!(
            msgpack_size <= json_size,
            "MessagePack should not be larger than JSON for {:?}",
            std::mem::discriminant(msg)
        );
    }
}
