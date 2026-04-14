import { useCallback, useEffect, useRef, useState } from 'react';
import type { InteractiveGameState, PlayerAction } from '@/types';
import { ServerMessageSchema } from '@/schemas';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected';

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

type ServerMessage =
  | { type: 'RoomState'; state: RoomLobbyState }
  | { type: 'GameState'; state: InteractiveGameState; turn_deadline_secs?: number | null }
  | { type: 'ActionApplied'; player: number; action: PlayerAction; state: InteractiveGameState; turn_deadline_secs?: number | null }
  | { type: 'BotAction'; player: number; action: PlayerAction; state: InteractiveGameState; turn_deadline_secs?: number | null }
  | { type: 'TimeoutAction'; player: number; action: PlayerAction; state: InteractiveGameState }
  | { type: 'PlayerJoined'; player_index: number; name: string }
  | { type: 'PlayerLeft'; player_index: number }
  | { type: 'PlayerReconnected'; player_index: number }
  | { type: 'Kicked'; reason: string }
  | { type: 'Error'; code: string; message: string }
  | { type: 'Pong' };

interface UseOnlineGameReturn {
  connectionStatus: ConnectionStatus;
  roomState: RoomLobbyState | null;
  gameState: InteractiveGameState | null;
  turnDeadlineSecs: number | null;
  wasTimeout: boolean;
  playerIndex: number | null;
  lastError: string | null;
  kicked: boolean;
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
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const reconnectAttemptRef = useRef(0);

  const send = useCallback((msg: object) => {
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg));
    }
  }, []);

  const connect = useCallback(() => {
    if (!roomCode || !sessionToken) return;

    setConnectionStatus('connecting');

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(
      `${protocol}//${window.location.host}/api/rooms/${roomCode}/ws?token=${sessionToken}`
    );

    ws.onopen = () => {
      setConnectionStatus('connected');
      setLastError(null);
      reconnectAttemptRef.current = 0;
    };

    ws.onmessage = (event) => {
      const msg: ServerMessage = ServerMessageSchema.parse(JSON.parse(event.data));

      switch (msg.type) {
        case 'RoomState':
          setRoomState(msg.state);
          setGameState(null);
          break;
        case 'GameState':
          setGameState(msg.state);
          setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
          setWasTimeout(false);
          break;
        case 'ActionApplied':
        case 'BotAction':
          setGameState(msg.state);
          setTurnDeadlineSecs(msg.turn_deadline_secs ?? null);
          setWasTimeout(false);
          break;
        case 'TimeoutAction':
          setGameState(msg.state);
          setTurnDeadlineSecs(null);
          setWasTimeout(true);
          break;
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
          break;
      }
    };

    ws.onclose = () => {
      setConnectionStatus('disconnected');
      wsRef.current = null;

      // Exponential backoff reconnection
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
  }, [roomCode, sessionToken]);

  // Connect when we have room code and token
  useEffect(() => {
    if (roomCode && sessionToken) {
      connect();
    }
    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
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

  const playAgain = useCallback(() => {
    send({ type: 'PlayAgain' });
  }, [send]);

  const returnToLobby = useCallback(() => {
    send({ type: 'ReturnToLobby' });
  }, [send]);

  const disconnect = useCallback(() => {
    reconnectAttemptRef.current = 999; // Prevent reconnect
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
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
    playAgain,
    returnToLobby,
    disconnect,
  };
}
