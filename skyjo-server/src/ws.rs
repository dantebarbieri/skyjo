use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use std::sync::Arc;

use crate::AppStateInner;
use crate::error::ServerError;
use crate::messages::{ClientMessage, PlayerSlotType, ServerMessage, WireFormat};
use crate::persistence::Persistence;
use crate::room::SharedRoom;

/// Stable machine-readable error code from variant name (strips data payloads).
fn error_code(err: &ServerError) -> String {
    let debug = format!("{:?}", err);
    debug
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect()
}

/// Convert a ServerError into a ServerMessage::Error.
fn error_msg(err: ServerError) -> ServerMessage {
    ServerMessage::Error {
        code: error_code(&err),
        message: err.message(),
    }
}

/// Handle a WebSocket connection for a player in a room.
#[allow(clippy::too_many_arguments)] // WebSocket setup requires all these contextual parameters
pub async fn handle_ws(
    ws: WebSocket,
    state: Arc<AppStateInner>,
    room: SharedRoom,
    room_code: String,
    player_index: usize,
    client_ip: String,
    initial_format: WireFormat,
    user_id: Option<uuid::Uuid>,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Parse IP for rate limiting
    let client_ip_addr: std::net::IpAddr = client_ip
        .parse()
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    // Wire format: use client's requested format, or default to JSON.
    // Auto-updates if client sends a different frame type later.
    let mut wire_format = initial_format;

    // Mark player as connected and record IP (never exposed to clients)
    let (player_msg_tx, mut player_msg_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let broadcast_rx = {
        let mut room_guard = room.lock().await;
        room_guard.players[player_index].connected = true;
        room_guard.players[player_index].ip = Some(client_ip);
        room_guard.players[player_index].disconnected_at = None;
        room_guard.players[player_index].user_id = user_id;

        // Register per-player targeted channel
        room_guard.set_player_tx(player_index, player_msg_tx);

        // If this player was converted to a bot while disconnected, convert back to human
        let was_reconverted = room_guard.reconnect_bot_to_human(player_index);

        room_guard.touch();

        // Send initial state
        let msg = match room_guard.phase {
            crate::room::RoomPhase::Lobby | crate::room::RoomPhase::GameOver => {
                ServerMessage::RoomState {
                    state: room_guard.lobby_state(),
                }
            }
            crate::room::RoomPhase::InGame => match room_guard.get_player_state(player_index) {
                Ok(state) => {
                    let turn_deadline_secs = room_guard.turn_deadline_secs();
                    let round_ready = room_guard.get_round_ready().cloned();
                    ServerMessage::GameState {
                        state,
                        turn_deadline_secs,
                        round_ready,
                    }
                }
                Err(_) => ServerMessage::RoomState {
                    state: room_guard.lobby_state(),
                },
            },
        };
        send_msg(&mut ws_tx, &msg, wire_format).await;

        // Notify others of reconnection
        for (i, slot) in room_guard.players.iter().enumerate() {
            if i != player_index && slot.connected {
                let _ = room_guard
                    .broadcast_tx
                    .send((i, ServerMessage::PlayerReconnected { player_index }));
            }
        }

        // If reconverted from bot, broadcast updated lobby/game state so others see the name change
        if was_reconverted {
            match room_guard.phase {
                crate::room::RoomPhase::InGame => {
                    room_guard.broadcast_game_state();
                }
                _ => {
                    room_guard.broadcast_lobby_state();
                }
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
                        wire_format = WireFormat::Json;
                        if !state.rate_limiter.check(
                            client_ip_addr,
                            "ws_message",
                            &crate::rate_limit::limits::WS_MESSAGE,
                        ) {
                            let _ = send_msg(
                                &mut ws_tx,
                                &ServerMessage::Error {
                                    code: "RateLimited".into(),
                                    message: "Too many messages".into(),
                                },
                                wire_format,
                            )
                            .await;
                            continue;
                        }
                        let response = handle_client_message(
                            &text,
                            &state,
                            &room,
                            &room_code,
                            player_index,
                        ).await;

                        if let Some(msg) = response {
                            send_msg(&mut ws_tx, &msg, wire_format).await;
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        wire_format = WireFormat::MessagePack;
                        if !state.rate_limiter.check(
                            client_ip_addr,
                            "ws_message",
                            &crate::rate_limit::limits::WS_MESSAGE,
                        ) {
                            let _ = send_msg(
                                &mut ws_tx,
                                &ServerMessage::Error {
                                    code: "RateLimited".into(),
                                    message: "Too many messages".into(),
                                },
                                wire_format,
                            )
                            .await;
                            continue;
                        }
                        let client_msg: ClientMessage = match rmp_serde::from_slice(&data) {
                            Ok(m) => m,
                            Err(e) => {
                                let err = ServerMessage::Error {
                                    code: "invalid_message".to_string(),
                                    message: format!("Failed to parse MessagePack message: {e}"),
                                };
                                send_msg(&mut ws_tx, &err, wire_format).await;
                                continue;
                            }
                        };
                        let response = handle_parsed_message(
                            client_msg,
                            &state,
                            &room,
                            &room_code,
                            player_index,
                        ).await;

                        if let Some(msg) = response {
                            send_msg(&mut ws_tx, &msg, wire_format).await;
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
                            send_msg(&mut ws_tx, &server_msg, wire_format).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Player {player_index} lagged {n} messages");
                        // Update lag count in room
                        {
                            let mut room_guard = room.lock().await;
                            room_guard.increment_broadcast_lag(player_index);
                        }
                        // Send fresh state to catch up
                        let room_guard = room.lock().await;
                        if let Ok(state) = room_guard.get_player_state(player_index) {
                            let turn_deadline_secs = room_guard.turn_deadline_secs();
                            let round_ready = room_guard.get_round_ready().cloned();
                            drop(room_guard);
                            send_msg(&mut ws_tx, &ServerMessage::GameState { state, turn_deadline_secs, round_ready }, wire_format).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Per-player targeted message (pre-serialized bytes)
            msg = player_msg_rx.recv() => {
                match msg {
                    Some(data) => {
                        let send_result = match wire_format {
                            WireFormat::Json => {
                                match String::from_utf8(data) {
                                    Ok(text) => ws_tx.send(Message::Text(text.into())).await,
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to send targeted JSON message to player {player_index}: invalid UTF-8: {e}"
                                        );
                                        continue;
                                    }
                                }
                            }
                            WireFormat::MessagePack => {
                                ws_tx.send(Message::Binary(data.into())).await
                            }
                        };
                        if send_result.is_err() {
                            break;
                        }
                    }
                    None => {
                        // Sender dropped — room is cleaning up
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
        room_guard.remove_player_tx(player_index);
        room_guard.touch();

        // Notify others
        for (i, slot) in room_guard.players.iter().enumerate() {
            if i != player_index && slot.connected {
                let _ = room_guard
                    .broadcast_tx
                    .send((i, ServerMessage::PlayerLeft { player_index }));
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

        // Schedule auto-kick or bot-conversion check for this player
        {
            let room_clone = room.clone();
            let state_ref = state.clone();
            let timeout = room_guard.effective_disconnect_bot_timeout();
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                let mut room_guard = room_clone.lock().await;
                match room_guard.phase {
                    crate::room::RoomPhase::InGame => {
                        // Convert disconnected players to bots instead of kicking
                        let converted = room_guard.convert_disconnected_to_bots(timeout);
                        for &slot in &converted {
                            let name = room_guard.players[slot].name.clone();
                            // Broadcast conversion notification to all connected players
                            for (i, p) in room_guard.players.iter().enumerate() {
                                if p.connected {
                                    let _ = room_guard.broadcast_tx.send((
                                        i,
                                        ServerMessage::PlayerConvertedToBot {
                                            slot,
                                            name: name.clone(),
                                        },
                                    ));
                                }
                            }
                        }
                        if !converted.is_empty() {
                            room_guard.broadcast_game_state();
                            // If it's now a bot's turn, run bot turns
                            if room_guard.is_current_player_bot() {
                                drop(room_guard);
                                let room_clone2 = room_clone.clone();
                                let persistence = state_ref.persistence.clone();
                                tokio::spawn(async move {
                                    run_bot_turns_with_persistence(room_clone2, Some(persistence))
                                        .await;
                                });
                            }
                        }
                    }
                    _ => {
                        // In Lobby or GameOver, kick as before
                        let kicked = room_guard.auto_kick_disconnected(timeout);
                        for (_, token) in &kicked {
                            if let Some(t) = token {
                                state_ref.lobby.sessions.remove(t);
                            }
                        }
                        if !kicked.is_empty() {
                            room_guard.broadcast_lobby_state();
                        }
                    }
                }
            });
        }
    }

    tracing::info!("Player {player_index} disconnected from room {room_code}");
}

async fn handle_client_message(
    text: &str,
    state: &Arc<AppStateInner>,
    room: &SharedRoom,
    room_code: &str,
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

    handle_parsed_message(msg, state, room, room_code, player_index).await
}

async fn handle_parsed_message(
    msg: ClientMessage,
    state: &Arc<AppStateInner>,
    room: &SharedRoom,
    room_code: &str,
    player_index: usize,
) -> Option<ServerMessage> {
    match msg {
        ClientMessage::Ping => Some(ServerMessage::Pong),

        ClientMessage::RequestFullState => {
            let room_guard = room.lock().await;
            match room_guard.get_player_state(player_index) {
                Ok(state) => {
                    let turn_deadline_secs = room_guard.turn_deadline_secs();
                    let round_ready = room_guard.get_round_ready().cloned();
                    Some(ServerMessage::GameState {
                        state,
                        turn_deadline_secs,
                        round_ready,
                    })
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::ConfigureSlot { slot, player_type } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.configure_slot(slot, &player_type) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::SetRules { rules } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.set_rules(&rules) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::SetNumPlayers { num_players } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.set_num_players(num_players) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::KickPlayer { slot } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.kick_player(slot) {
                Ok(token) => {
                    // Clean up the kicked player's session
                    if let Some(t) = token {
                        state.lobby.sessions.remove(&t);
                    }
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::BanPlayer { slot } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.ban_player(slot) {
                Ok(token) => {
                    if let Some(t) = token {
                        state.lobby.sessions.remove(&t);
                    }
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::StartGame => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            // Snapshot the genetic genome if any player uses a Genetic strategy
            let has_genetic = room_guard.players.iter().any(|p| {
                matches!(&p.slot_type, crate::messages::PlayerSlotType::Bot { strategy } if strategy.starts_with("Genetic"))
            });
            if has_genetic {
                let genetic = state.genetic.lock().await;
                // For "Genetic" (latest), use best genome
                // For "Genetic:name" (saved), resolve from saved generations
                // We store the latest genome by default; saved generation lookups
                // happen in apply_bot_action via the strategy name prefix
                room_guard.genetic_genome = Some(genetic.best_genome.clone());
                room_guard.genetic_games_trained = genetic.total_games_trained;
                room_guard.genetic_generation = genetic.generation;

                // If any player uses a saved generation, resolve its genome
                let saved_name: Option<String> = room_guard.players.iter().find_map(|p| {
                    if let crate::messages::PlayerSlotType::Bot { strategy } = &p.slot_type {
                        strategy.strip_prefix("Genetic:").map(|s| s.to_string())
                    } else {
                        None
                    }
                });
                if let Some(name) = saved_name
                    && let Some((genome, games)) = genetic.get_saved_genome(&name)
                {
                    room_guard.genetic_genome = Some(genome);
                    room_guard.genetic_games_trained = games;
                }
            }

            match room_guard.start_game() {
                Ok(()) => {
                    room_guard.broadcast_game_state();

                    // Persist game start
                    let game_id = room_guard.game_id;
                    let game_seed = room_guard.game_seed;
                    let rules_name = room_guard.rules_name.clone();
                    let room_code_owned = room_code.to_string();
                    let players = extract_player_data(&room_guard);

                    // Schedule bot turns if the first player is a bot
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        let persistence_clone = state.persistence.clone();
                        tokio::spawn(async move {
                            run_bot_turns_with_persistence(room_clone, Some(persistence_clone))
                                .await;
                        });
                    } else {
                        drop(room_guard);
                        schedule_turn_timeout_with_persistence(
                            room.clone(),
                            Some(state.persistence.clone()),
                        );
                    }

                    // Persist in background (don't block game flow)
                    if let (Some(gid), Some(seed)) = (game_id, game_seed) {
                        let persistence = state.persistence.clone();
                        tokio::spawn(async move {
                            persist_game_start(
                                &persistence,
                                gid,
                                &room_code_owned,
                                &rules_name,
                                seed,
                                &players,
                            )
                            .await;
                        });
                    }

                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::Action { action } => {
            let mut room_guard = room.lock().await;

            // Capture timing before applying
            let elapsed_before = room_guard.elapsed_since_turn_start();

            if let Some(elapsed) = elapsed_before
                && elapsed < Duration::from_millis(100)
            {
                tracing::warn!(
                    room = %room_code,
                    player = player_index,
                    elapsed_ms = elapsed.as_millis(),
                    "suspiciously_fast_action"
                );
            }

            match room_guard.apply_action(player_index, action.clone()) {
                Ok(delta) => {
                    tracing::info!(
                        room = %room_code,
                        player = player_index,
                        action = ?action,
                        elapsed_since_turn_start_ms = elapsed_before.map(|d| d.as_millis()),
                        "action_applied"
                    );

                    room_guard.broadcast_action(player_index, &action, false, &delta);

                    // Check if game just ended
                    let game_over_data = if room_guard.phase == crate::room::RoomPhase::GameOver {
                        room_guard.game_id.map(|id| {
                            let history = room_guard.game.as_ref().map(|g| g.build_history());
                            let players = extract_player_data(&room_guard);
                            (id, history, players)
                        })
                    } else {
                        None
                    };

                    // Schedule bot turns if the next player is a bot
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        let persistence_clone = state.persistence.clone();
                        tokio::spawn(async move {
                            run_bot_turns_with_persistence(room_clone, Some(persistence_clone))
                                .await;
                        });
                    } else {
                        drop(room_guard);
                        schedule_turn_timeout_with_persistence(
                            room.clone(),
                            Some(state.persistence.clone()),
                        );
                    }

                    // Persist game completion in background
                    if let Some((game_id, history, players)) = game_over_data {
                        let persistence = state.persistence.clone();
                        tokio::spawn(async move {
                            persist_game_completion(
                                &persistence,
                                game_id,
                                history.as_ref(),
                                &players,
                            )
                            .await;
                        });
                    }

                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::SetReady { ready } => {
            let mut room_guard = room.lock().await;

            match room_guard.set_ready(player_index, ready) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::ReadyForNextRound | ClientMessage::ContinueRound => {
            handle_round_ready(player_index, state, room).await
        }

        ClientMessage::PlayAgain => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.play_again() {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::ReturnToLobby => {
            let mut room_guard = room.lock().await;

            match room_guard.return_to_lobby() {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::SetTurnTimer { secs } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.set_turn_timer(secs) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::SetDisconnectTimeout { secs } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.set_disconnect_bot_timeout(secs) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }

        ClientMessage::PromoteHost { slot } => {
            let mut room_guard = room.lock().await;

            if player_index != room_guard.creator {
                return Some(error_msg(ServerError::NotHost));
            }

            match room_guard.promote_host(slot) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    None
                }
                Err(e) => Some(error_msg(e)),
            }
        }
    }
}

/// Handle a player signaling readiness for the next round.
/// Shared by `ReadyForNextRound` and `ContinueRound` (backward compat).
async fn handle_round_ready(
    player_index: usize,
    state: &Arc<AppStateInner>,
    room: &SharedRoom,
) -> Option<ServerMessage> {
    let mut room_guard = room.lock().await;

    match room_guard.set_round_ready(player_index) {
        Ok(all_ready) => {
            if all_ready {
                match room_guard.continue_round() {
                    Ok(()) => {
                        room_guard.broadcast_game_state();

                        if room_guard.is_current_player_bot() {
                            drop(room_guard);
                            let room_clone = room.clone();
                            let persistence_clone = state.persistence.clone();
                            tokio::spawn(async move {
                                run_bot_turns_with_persistence(room_clone, Some(persistence_clone))
                                    .await;
                            });
                        } else {
                            drop(room_guard);
                            schedule_turn_timeout_with_persistence(
                                room.clone(),
                                Some(state.persistence.clone()),
                            );
                        }

                        None
                    }
                    Err(e) => Some(error_msg(e)),
                }
            } else {
                // Not all ready yet — broadcast updated round_ready state
                room_guard.broadcast_game_state();
                None
            }
        }
        Err(e) => Some(error_msg(e)),
    }
}

/// Schedule a turn timeout check, with optional persistence for game-over handling.
fn schedule_turn_timeout_with_persistence(room: SharedRoom, persistence: Option<Persistence>) {
    tokio::spawn(async move {
        // Read the timer duration
        let timer_secs = {
            let room_guard = room.lock().await;
            match room_guard.turn_timer_secs {
                Some(s) => s,
                None => return,
            }
        };

        // Sleep for the full timer duration + 1s buffer for timing jitter
        tokio::time::sleep(Duration::from_secs(timer_secs + 1)).await;

        let mut room_guard = room.lock().await;

        // Check and apply timeout
        match room_guard.check_turn_timeout() {
            Ok(Some((player, action, delta))) => {
                room_guard.broadcast_timeout_action(player, &action, &delta);

                // Check if the timeout action ended the game
                let game_over_data = if room_guard.phase == crate::room::RoomPhase::GameOver {
                    room_guard.game_id.map(|id| {
                        let history = room_guard.game.as_ref().map(|g| g.build_history());
                        let players = extract_player_data(&room_guard);
                        (id, history, players)
                    })
                } else {
                    None
                };

                // If the next player is a bot, run bot turns
                if room_guard.is_current_player_bot() {
                    drop(room_guard);
                    let room_clone = room.clone();
                    let p = persistence.clone();
                    tokio::spawn(async move {
                        run_bot_turns_with_persistence(room_clone, p).await;
                    });
                } else {
                    // Schedule next timeout for the next human player
                    drop(room_guard);
                    schedule_turn_timeout_with_persistence(room.clone(), persistence.clone());
                }

                // Persist game completion if it ended
                if let Some((game_id, history, players)) = game_over_data
                    && let Some(persistence) = &persistence
                {
                    let persistence = persistence.clone();
                    tokio::spawn(async move {
                        persist_game_completion(&persistence, game_id, history.as_ref(), &players)
                            .await;
                    });
                }
            }
            Ok(None) => {
                // No timeout — turn was already completed or timer not active
            }
            Err(e) => {
                tracing::error!("Turn timeout check failed: {e}");
            }
        }
    });
}

/// Run consecutive bot turns, optionally persisting game-over.
async fn run_bot_turns_with_persistence(room: SharedRoom, persistence: Option<Persistence>) {
    loop {
        // Delay for natural pacing
        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut room_guard = room.lock().await;

        if !room_guard.is_current_player_bot() {
            // Human player's turn — schedule timeout
            drop(room_guard);
            schedule_turn_timeout_with_persistence(room.clone(), persistence.clone());
            break;
        }

        match room_guard.apply_bot_action() {
            Ok((bot_player, action, delta)) => {
                room_guard.broadcast_action(bot_player, &action, true, &delta);

                // Check if game just ended
                if room_guard.phase == crate::room::RoomPhase::GameOver {
                    if let (Some(persistence), Some(game_id)) = (&persistence, room_guard.game_id) {
                        let history = room_guard.game.as_ref().map(|g| g.build_history());
                        let players = extract_player_data(&room_guard);
                        let persistence = persistence.clone();
                        drop(room_guard);
                        tokio::spawn(async move {
                            persist_game_completion(
                                &persistence,
                                game_id,
                                history.as_ref(),
                                &players,
                            )
                            .await;
                        });
                    }
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Bot action failed: {e}");
                break;
            }
        }
    }
}

async fn send_msg(tx: &mut SplitSink<WebSocket, Message>, msg: &ServerMessage, format: WireFormat) {
    let ws_msg = match format {
        WireFormat::Json => match serde_json::to_string(msg) {
            Ok(json) => Message::Text(json.into()),
            Err(_) => return,
        },
        WireFormat::MessagePack => match rmp_serde::to_vec_named(msg) {
            Ok(bytes) => Message::Binary(bytes.into()),
            Err(_) => return,
        },
    };
    let _ = tx.send(ws_msg).await;
}

/// Persist a newly started game (in_progress state) with its players atomically.
async fn persist_game_start(
    persistence: &Persistence,
    game_id: uuid::Uuid,
    room_code: &str,
    rules_name: &str,
    seed: u64,
    players: &[(usize, String, Option<uuid::Uuid>, Option<String>)],
) {
    let player_refs: Vec<(usize, &str, Option<uuid::Uuid>, Option<&str>)> = players
        .iter()
        .map(|(idx, name, user_id, strategy)| (*idx, name.as_str(), *user_id, strategy.as_deref()))
        .collect();

    if let Err(e) = persistence
        .save_game_with_players(
            game_id,
            room_code,
            rules_name,
            Some(seed as i64),
            &player_refs,
        )
        .await
    {
        tracing::error!(game_id = %game_id, "Failed to persist game start: {e}");
    }
}

/// Save full game history and mark as completed.
async fn persist_game_completion(
    persistence: &Persistence,
    game_id: uuid::Uuid,
    history: Option<&skyjo_core::GameHistory>,
    _players: &[(usize, String, Option<uuid::Uuid>, Option<String>)],
) {
    if let Some(history) = history {
        // Only mark as completed if history save succeeds
        match persistence
            .save_game_history(game_id, &history.rules_name, history)
            .await
        {
            Ok(()) => {
                if let Err(e) = persistence.update_game_state(game_id, "completed").await {
                    tracing::error!(game_id = %game_id, "Failed to mark game as completed: {e}");
                }
            }
            Err(e) => {
                tracing::error!(game_id = %game_id, "Failed to persist game history: {e}");
                // Don't mark as completed — game stays in_progress for retry
            }
        }
    } else {
        // No history available — game had no rounds; leave as in_progress for investigation
        tracing::warn!(game_id = %game_id, "persist_game_completion called with no history — leaving as in_progress");
    }
}

/// Extract player data from a room for persistence.
fn extract_player_data(
    room: &crate::room::Room,
) -> Vec<(usize, String, Option<uuid::Uuid>, Option<String>)> {
    room.players
        .iter()
        .enumerate()
        .take(room.num_players)
        .filter(|(_, p)| p.slot_type != PlayerSlotType::Empty)
        .map(|(idx, p)| {
            let strategy = match &p.slot_type {
                PlayerSlotType::Bot { strategy } => Some(strategy.clone()),
                _ => None,
            };
            (idx, p.name.clone(), p.user_id, strategy)
        })
        .collect()
}
