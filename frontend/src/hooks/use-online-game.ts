import { useCallback, useEffect, useRef, useState } from 'react';
import type { InteractiveGameState, PlayerAction } from '@/types';
import { ServerMessageSchema } from '@/schemas';
import type { PendingColumnClear } from './use-interactive-game';
import { useAuth } from '@/contexts/auth-context';
import type { z } from 'zod';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected';

/**
 * Custom WebSocket close code (RFC 6455 §7.4.2 application range 4000–4999).
 * The server does not currently send this code, but the client handles it
 * defensively in case future server logic actively closes sockets on session
 * invalidation. The primary detection mechanism is the HTTP session validation
 * fetch before each reconnect attempt.
 */
const WS_CLOSE_SESSION_EXPIRED = 4001;

const COLUMN_CLEAR_DELAY_MS = 2500;

/**
 * Build an intermediate "pre-clear" state from a post-clear state.
 * Replaces Cleared slots in the clearing columns with Revealed(card_value).
 */
function buildPreClearState(state: InteractiveGameState): InteractiveGameState {
  const { last_column_clears, num_rows } = state;
  if (last_column_clears.length === 0) return state;

  const boards = state.boards.map(b => [...b]);
  for (const clear of last_column_clears) {
    for (let r = 0; r < num_rows; r++) {
      const idx = clear.column * num_rows + r;
      if (boards[clear.player_index][idx] === 'Cleared') {
        boards[clear.player_index][idx] = { Revealed: clear.card_value };
      }
    }
  }
  return { ...state, boards, last_column_clears: [] };
}

export interface RoomLobbyState {
  room_code: string;
  players: LobbyPlayer[];
  num_players: number;
  rules: string;
  creator: number;
  available_strategies: string[];
  available_rules: string[];
  idle_timeout_secs: number | null;
  turn_timer_secs: number | null;
  last_winners: number[];
  genetic_games_trained: number;
  genetic_generation: number;
}

export interface LobbyPlayer {
  slot: number;
  name: string;
  player_type: PlayerSlotType;
  connected: boolean;
  shares_ip_with_host?: boolean;
  disconnect_secs?: number;
}

export type PlayerSlotType =
  | { kind: 'Human' }
  | { kind: 'Bot'; strategy: string }
  | { kind: 'Empty' };

type ServerMessage = z.infer<typeof ServerMessageSchema>;

interface UseOnlineGameReturn {
  connectionStatus: ConnectionStatus;
  roomState: RoomLobbyState | null;
  gameState: InteractiveGameState | null;
  turnDeadlineSecs: number | null;
  wasTimeout: boolean;
  playerIndex: number | null;
  lastError: string | null;
  kicked: boolean;
  sessionExpired: boolean;
  pendingClearColumns: PendingColumnClear[] | null;
  roundReady: boolean[] | null;
  applyAction: (action: PlayerAction) => void;
  configureSlot: (slot: number, playerType: string) => void;
  setNumPlayers: (numPlayers: number) => void;
  setRules: (rules: string) => void;
  setTurnTimer: (secs: number | null) => void;
  kickPlayer: (slot: number) => void;
  banPlayer: (slot: number) => void;
  promoteHost: (slot: number) => void;
  startGame: () => void;
  continueRound: () => void;
  readyForNextRound: () => void;
  setReady: (ready: boolean) => void;
  playAgain: () => void;
  returnToLobby: () => void;
  disconnect: () => void;
}

export function useOnlineGame(
  roomCode: string | null,
  sessionToken: string | null,
  playerIndex: number | null,
): UseOnlineGameReturn {
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('disconnected');
  const [roomState, setRoomState] = useState<RoomLobbyState | null>(null);
  const [gameState, setGameState] = useState<InteractiveGameState | null>(null);
  const [turnDeadlineSecs, setTurnDeadlineSecs] = useState<number | null>(null);
  const [wasTimeout, setWasTimeout] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const [kicked, setKicked] = useState(false);
  const [sessionExpired, setSessionExpired] = useState(false);
  const [pendingClearColumns, setPendingClearColumns] = useState<PendingColumnClear[] | null>(null);
  const [roundReady, setRoundReady] = useState<boolean[] | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const reconnectAttemptRef = useRef(0);
  const pendingClearTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const connectAbortRef = useRef<AbortController | null>(null);
  const { accessToken } = useAuth();

  const send = useCallback((msg: object) => {
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg));
    }
  }, []);

  const connect = useCallback(() => {
    if (!roomCode || !sessionToken) return;

    // Cancel any in-flight validation fetch from a previous connect attempt
    connectAbortRef.current?.abort();
    const abortController = new AbortController();
    connectAbortRef.current = abortController;

    setConnectionStatus('connecting');

    // Validate the session before attempting WebSocket connection.
    // This detects stale tokens (e.g. after server restart) without relying on
    // browser WebSocket error codes, which don't expose HTTP status.
    const encodedToken = encodeURIComponent(sessionToken);
    fetch(`/api/rooms/${roomCode}/validate-session?token=${encodedToken}`, {
      signal: abortController.signal,
    })
      .then((res) => {
        if (abortController.signal.aborted) return;

        if (res.status === 401) {
          // Session is invalid — stop reconnecting
          setSessionExpired(true);
          setConnectionStatus('disconnected');
          reconnectAttemptRef.current = 999;
          return;
        }

        if (!res.ok) {
          // Non-auth error (404, 500, etc.) — treat as transient, retry with backoff
          setConnectionStatus('disconnected');
          const attempt = reconnectAttemptRef.current;
          if (attempt < 10) {
            const delay = Math.min(1000 * Math.pow(2, attempt), 30000);
            reconnectTimeoutRef.current = setTimeout(() => {
              reconnectAttemptRef.current++;
              connect();
            }, delay);
          }
          return;
        }

        if (abortController.signal.aborted) return;

        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        let wsUrl = `${protocol}//${window.location.host}/api/rooms/${roomCode}/ws?token=${encodedToken}`;
        if (accessToken) {
          wsUrl += `&access_token=${encodeURIComponent(accessToken)}`;
        }
        const ws = new WebSocket(wsUrl);

        ws.onopen = () => {
          setConnectionStatus('connected');
          setLastError(null);
          reconnectAttemptRef.current = 0;
        };

        ws.onmessage = (event) => {
          let rawMessage: unknown;

          try {
            rawMessage = JSON.parse(event.data);
          } catch {
            setLastError('Received invalid JSON from server.');
            return;
          }

          const parsedMessage = ServerMessageSchema.safeParse(rawMessage);
          if (!parsedMessage.success) {
            if (import.meta.env.DEV) {
              console.error('Invalid server message:', JSON.stringify(rawMessage).slice(0, 500), parsedMessage.error.issues);
            }
            setLastError('Received invalid server message.');
            return;
          }

          const msg: ServerMessage = parsedMessage.data;

          switch (msg.type) {
            case 'RoomState':
              setRoomState(msg.state);
              setGameState(null);
              setRoundReady(null);
              break;
            case 'GameState':
              if (pendingClearTimeoutRef.current) {
                clearTimeout(pendingClearTimeoutRef.current);
                pendingClearTimeoutRef.current = null;
              }
              setPendingClearColumns(null);
              setGameState(msg.state);
              setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
              setRoundReady(msg.round_ready ?? null);
              setWasTimeout(false);
              break;
            case 'ActionApplied':
            case 'BotAction': {
              setRoundReady(msg.round_ready ?? null);
              const isFlipClear = msg.action.type === 'DiscardAndFlip' && msg.state.last_column_clears.length > 0;
              if (isFlipClear) {
                const preClearState = buildPreClearState(msg.state);
                const clearCols = msg.state.last_column_clears.map(c => ({
                  playerIndex: c.player_index,
                  column: c.column,
                }));
                setPendingClearColumns(clearCols);
                setGameState(preClearState);
                setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
                setWasTimeout(false);
                if (pendingClearTimeoutRef.current) clearTimeout(pendingClearTimeoutRef.current);
                pendingClearTimeoutRef.current = setTimeout(() => {
                  setPendingClearColumns(null);
                  setGameState(msg.state);
                  pendingClearTimeoutRef.current = null;
                }, COLUMN_CLEAR_DELAY_MS);
              } else {
                setGameState(msg.state);
                setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
                setWasTimeout(false);
              }
              break;
            }
            case 'TimeoutAction': {
              const isFlipClearTimeout = msg.action.type === 'DiscardAndFlip' && msg.state.last_column_clears.length > 0;
              if (isFlipClearTimeout) {
                const preClearState = buildPreClearState(msg.state);
                const clearCols = msg.state.last_column_clears.map(c => ({
                  playerIndex: c.player_index,
                  column: c.column,
                }));
                setPendingClearColumns(clearCols);
                setGameState(preClearState);
                setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
                setWasTimeout(true);
                if (pendingClearTimeoutRef.current) clearTimeout(pendingClearTimeoutRef.current);
                pendingClearTimeoutRef.current = setTimeout(() => {
                  setPendingClearColumns(null);
                  setGameState(msg.state);
                  pendingClearTimeoutRef.current = null;
                }, COLUMN_CLEAR_DELAY_MS);
              } else {
                setGameState(msg.state);
                setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
                setWasTimeout(true);
              }
              break;
            }
            case 'PlayerJoined':
              setRoomState(prev => {
                if (!prev) return prev;
                const players = [...prev.players];
                players[msg.player_index] = {
                  ...players[msg.player_index],
                  name: msg.name,
                  player_type: { kind: 'Human' },
                  connected: true,
                };
                return { ...prev, players };
              });
              break;
            case 'PlayerLeft':
              setRoomState(prev => {
                if (!prev) return prev;
                const players = [...prev.players];
                players[msg.player_index] = {
                  ...players[msg.player_index],
                  connected: false,
                };
                return { ...prev, players };
              });
              break;
            case 'PlayerReconnected':
              setRoomState(prev => {
                if (!prev) return prev;
                const players = [...prev.players];
                players[msg.player_index] = {
                  ...players[msg.player_index],
                  connected: true,
                };
                return { ...prev, players };
              });
              break;
            case 'Kicked':
              setKicked(true);
              setLastError(msg.reason);
              // Stop reconnecting
              reconnectAttemptRef.current = 999;
              break;
            case 'Error':
              setLastError(msg.message);
              break;
            case 'Pong':
            case 'ActionAppliedDelta':
              // Delta messages are ignored — we use the full state from ActionApplied/BotAction
              break;
            case 'PlayerConvertedToBot':
              setRoomState(prev => {
                if (!prev) return prev;
                const players = [...prev.players];
                players[msg.slot] = {
                  ...players[msg.slot],
                  name: msg.name,
                  player_type: { kind: 'Bot', strategy: 'Random' },
                  connected: false,
                };
                return { ...prev, players };
              });
              break;
            case 'ServerShutdown':
              setLastError('Server is shutting down. Please reconnect shortly.');
              break;
          }
        };

        ws.onclose = (event) => {
          setConnectionStatus('disconnected');
          wsRef.current = null;

          // Session expired close code — stop reconnecting immediately
          if (event.code === WS_CLOSE_SESSION_EXPIRED) {
            setSessionExpired(true);
            reconnectAttemptRef.current = 999;
            return;
          }

          // Exponential backoff reconnection for network failures
          const attempt = reconnectAttemptRef.current;
          if (attempt < 10) {
            const delay = Math.min(1000 * Math.pow(2, attempt), 30000);
            reconnectTimeoutRef.current = setTimeout(() => {
              reconnectAttemptRef.current++;
              connect();
            }, delay);
          }
        };

        ws.onerror = () => {
          // onclose will fire after this
        };

        wsRef.current = ws;
      })
      .catch((err) => {
        // Ignore aborted requests (from cleanup/disconnect)
        if (err instanceof DOMException && err.name === 'AbortError') return;
        // Network error on validation fetch — treat as temporary, retry with backoff
        setConnectionStatus('disconnected');
        const attempt = reconnectAttemptRef.current;
        if (attempt < 10) {
          const delay = Math.min(1000 * Math.pow(2, attempt), 30000);
          reconnectTimeoutRef.current = setTimeout(() => {
            reconnectAttemptRef.current++;
            connect();
          }, delay);
        }
      });
  }, [roomCode, sessionToken, accessToken]);

  // Connect when we have room code and token
  useEffect(() => {
    if (roomCode && sessionToken) {
      connect();
    }
    return () => {
      connectAbortRef.current?.abort();
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (pendingClearTimeoutRef.current) {
        clearTimeout(pendingClearTimeoutRef.current);
        pendingClearTimeoutRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [roomCode, sessionToken, connect]);

  // Keepalive ping every 30 seconds
  useEffect(() => {
    if (connectionStatus !== 'connected') return;
    const interval = setInterval(() => {
      send({ type: 'Ping' });
    }, 30000);
    return () => clearInterval(interval);
  }, [connectionStatus, send]);

  const applyAction = useCallback((action: PlayerAction) => {
    send({ type: 'Action', action });
  }, [send]);

  const configureSlot = useCallback((slot: number, playerType: string) => {
    send({ type: 'ConfigureSlot', slot, player_type: playerType });
  }, [send]);

  const setNumPlayers = useCallback((numPlayers: number) => {
    send({ type: 'SetNumPlayers', num_players: numPlayers });
  }, [send]);

  const setRules = useCallback((rules: string) => {
    send({ type: 'SetRules', rules });
  }, [send]);

  const setTurnTimer = useCallback((secs: number | null) => {
    send({ type: 'SetTurnTimer', secs });
  }, [send]);

  const kickPlayer = useCallback((slot: number) => {
    send({ type: 'KickPlayer', slot });
  }, [send]);

  const banPlayer = useCallback((slot: number) => {
    send({ type: 'BanPlayer', slot });
  }, [send]);

  const promoteHost = useCallback((slot: number) => {
    send({ type: 'PromoteHost', slot });
  }, [send]);

  const startGame = useCallback(() => {
    send({ type: 'StartGame' });
  }, [send]);

  const continueRound = useCallback(() => {
    send({ type: 'ContinueRound' });
  }, [send]);

  const readyForNextRound = useCallback(() => {
    send({ type: 'ReadyForNextRound' });
  }, [send]);

  const setReady = useCallback((ready: boolean) => {
    send({ type: 'SetReady', ready });
  }, [send]);

  const playAgain = useCallback(() => {
    send({ type: 'PlayAgain' });
  }, [send]);

  const returnToLobby = useCallback(() => {
    send({ type: 'ReturnToLobby' });
  }, [send]);

  const disconnect = useCallback(() => {
    reconnectAttemptRef.current = 999; // Prevent reconnect
    connectAbortRef.current?.abort();
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }
    if (pendingClearTimeoutRef.current) {
      clearTimeout(pendingClearTimeoutRef.current);
      pendingClearTimeoutRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setConnectionStatus('disconnected');
    setRoomState(null);
    setGameState(null);
    setLastError(null);
    setKicked(false);
    setSessionExpired(false);
    setPendingClearColumns(null);
    setRoundReady(null);
  }, []);

  return {
    connectionStatus,
    roomState,
    gameState,
    turnDeadlineSecs,
    wasTimeout,
    playerIndex,
    lastError,
    kicked,
    sessionExpired,
    pendingClearColumns,
    roundReady,
    applyAction,
    configureSlot,
    setNumPlayers,
    setRules,
    setTurnTimer,
    kickPlayer,
    banPlayer,
    promoteHost,
    startGame,
    continueRound,
    readyForNextRound,
    setReady,
    playAgain,
    returnToLobby,
    disconnect,
  };
}
