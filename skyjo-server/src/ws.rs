use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use std::sync::Arc;

use crate::lobby::Lobby;
use crate::messages::{ClientMessage, ServerMessage};
use crate::room::SharedRoom;

/// Handle a WebSocket connection for a player in a room.
pub async fn handle_ws(
    ws: WebSocket,
    lobby: Arc<Lobby>,
    room: SharedRoom,
    room_code: String,
    player_index: usize,
    client_ip: String,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Mark player as connected and record IP (never exposed to clients)
    let broadcast_rx = {
        let mut room_guard = room.lock().await;
        room_guard.players[player_index].connected = true;
        room_guard.players[player_index].ip = Some(client_ip);
        room_guard.players[player_index].disconnected_at = None;
        room_guard.touch();

        // Send initial state
        let msg = match room_guard.phase {
            crate::room::RoomPhase::Lobby | crate::room::RoomPhase::GameOver => {
                ServerMessage::RoomState {
                    state: room_guard.lobby_state(),
                }
            }
            crate::room::RoomPhase::InGame => {
                match room_guard.get_player_state(player_index) {
                    Ok(state) => ServerMessage::GameState { state },
                    Err(_) => ServerMessage::RoomState {
                        state: room_guard.lobby_state(),
                    },
                }
            }
        };
        send_msg(&mut ws_tx, &msg).await;

        // Notify others of reconnection
        for (i, slot) in room_guard.players.iter().enumerate() {
            if i != player_index && slot.connected {
                let _ = room_guard.broadcast_tx.send((
                    i,
                    ServerMessage::PlayerReconnected { player_index },
                ));
            }
        }

        // Subscribe to broadcast channel
        room_guard.broadcast_tx.subscribe()
    };

    let mut broadcast_rx = broadcast_rx;

    // Main message loop: handle both incoming WS messages and broadcast messages
    loop {
        tokio::select! {
            // Incoming WebSocket message from client
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = handle_client_message(
                            &text,
                            &lobby,
                            &room,
                            player_index,
                        ).await;

                        if let Some(msg) = response {
                            send_msg(&mut ws_tx, &msg).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
            // Broadcast message for this player
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok((target_player, server_msg)) => {
                        if target_player == player_index {
                            send_msg(&mut ws_tx, &server_msg).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Player {player_index} lagged {n} messages");
                        // Send fresh state to catch up
                        let room_guard = room.lock().await;
                        if let Ok(state) = room_guard.get_player_state(player_index) {
                            drop(room_guard);
                            send_msg(&mut ws_tx, &ServerMessage::GameState { state }).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    // Player disconnected
    {
        let mut room_guard = room.lock().await;
        room_guard.players[player_index].connected = false;
        room_guard.players[player_index].disconnected_at = Some(std::time::Instant::now());
        room_guard.touch();

        // Notify others
        for (i, slot) in room_guard.players.iter().enumerate() {
            if i != player_index && slot.connected {
                let _ = room_guard.broadcast_tx.send((
                    i,
                    ServerMessage::PlayerLeft { player_index },
                ));
            }
        }

        // Schedule auto-promote host if the disconnected player was the host
        if player_index == room_guard.creator {
            let room_clone = room.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let mut room_guard = room_clone.lock().await;
                if room_guard.auto_promote_host() {
                    room_guard.broadcast_lobby_state();
                }
            });
        }

        // Schedule auto-kick check for this player
        {
            let room_clone = room.clone();
            let lobby_ref = lobby.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let mut room_guard = room_clone.lock().await;
                let kicked = room_guard.auto_kick_disconnected(Duration::from_secs(60));
                for (_, token) in &kicked {
                    if let Some(t) = token {
                        lobby_ref.sessions.remove(t);
                    }
                }
                if !kicked.is_empty() {
                    room_guard.broadcast_lobby_state();
                }
            });
        }
    }

    tracing::info!("Player {player_index} disconnected from room {room_code}");
}

async fn handle_client_message(
    text: &str,
    lobby: &Lobby,
    room: &SharedRoom,
    player_index: usize,
) -> Option<ServerMessage> {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            return Some(ServerMessage::Error {
                code: "invalid_message".to_string(),
                message: format!("Failed to parse message: {e}"),
            });
        }
    };

    match msg {
        ClientMessage::Ping => Some(ServerMessage::Pong),

        ClientMessage::ConfigureSlot { slot, player_type } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can configure slots".to_string(),
                });
            }

            match room_guard.configure_slot(slot, &player_type) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "configure_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::SetRules { rules } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can change rules".to_string(),
                });
            }

            match room_guard.set_rules(&rules) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "set_rules_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::SetNumPlayers { num_players } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can change player count".to_string(),
                });
            }

            match room_guard.set_num_players(num_players) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "set_players_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::KickPlayer { slot } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can kick players".to_string(),
                });
            }

            match room_guard.kick_player(slot) {
                Ok(token) => {
                    // Clean up the kicked player's session
                    if let Some(t) = token {
                        lobby.sessions.remove(&t);
                    }
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "kick_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::BanPlayer { slot } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can ban players".to_string(),
                });
            }

            match room_guard.ban_player(slot) {
                Ok(token) => {
                    if let Some(t) = token {
                        lobby.sessions.remove(&t);
                    }
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "ban_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::StartGame => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can start the game".to_string(),
                });
            }

            match room_guard.start_game() {
                Ok(()) => {
                    room_guard.broadcast_game_state();

                    // Schedule bot turns if the first player is a bot
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        tokio::spawn(async move {
                            run_bot_turns(room_clone).await;
                        });
                    }

                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "start_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::Action { action } => {
            let mut room_guard = room.lock().await;

            match room_guard.apply_action(player_index, action.clone()) {
                Ok(()) => {
                    room_guard.broadcast_action(player_index, &action, false);

                    // Schedule bot turns if the next player is a bot
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        tokio::spawn(async move {
                            run_bot_turns(room_clone).await;
                        });
                    }

                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "action_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::ContinueRound => {
            let mut room_guard = room.lock().await;

            match room_guard.continue_round() {
                Ok(()) => {
                    room_guard.broadcast_game_state();

                    // Schedule bot turns if the first player of the new round is a bot
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        tokio::spawn(async move {
                            run_bot_turns(room_clone).await;
                        });
                    }

                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "continue_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::PlayAgain => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can restart the game".to_string(),
                });
            }

            match room_guard.play_again() {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "play_again_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::ReturnToLobby => {
            let mut room_guard = room.lock().await;

            match room_guard.return_to_lobby() {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "return_error".to_string(),
                    message: e,
                }),
            }
        }

        ClientMessage::PromoteHost { slot } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(ServerMessage::Error {
                    code: "not_creator".to_string(),
                    message: "Only the room creator can promote players".to_string(),
                });
            }

            match room_guard.promote_host(slot) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(ServerMessage::Error {
                    code: "promote_error".to_string(),
                    message: e,
                }),
            }
        }
    }
}

/// Run consecutive bot turns with delays until a human player's turn or game end.
async fn run_bot_turns(room: SharedRoom) {
    loop {
        // Delay for natural pacing
        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut room_guard = room.lock().await;

        if !room_guard.is_current_player_bot() {
            break;
        }

        match room_guard.apply_bot_action() {
            Ok((bot_player, action)) => {
                room_guard.broadcast_action(bot_player, &action, true);
            }
            Err(e) => {
                tracing::error!("Bot action failed: {e}");
                break;
            }
        }
    }
}

async fn send_msg(tx: &mut SplitSink<WebSocket, Message>, msg: &ServerMessage) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = tx.send(Message::Text(json.into())).await;
    }
}
