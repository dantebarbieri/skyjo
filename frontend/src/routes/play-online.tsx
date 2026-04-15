import { useCallback, useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent } from '@/components/ui/card';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import { ActionButtons } from '@/components/action-buttons';
import SkyjoCard from '@/components/skyjo-card';
import { RoundScorecard } from '@/components/round-scorecard';
import { useResponsiveCardSize } from '@/hooks/use-responsive-card-size';
import { cn } from '@/lib/utils';
import { toSlot, getPlayerName, computeVisibleScore } from '@/lib/game-helpers';
import { getCardColorGroup, COLUMN_CLEAR_COLORS } from '@/lib/card-styles';
import { useAuth } from '@/contexts/auth-context';
import {
  useOnlineGame,
  type RoomLobbyState,
  type ConnectionStatus,
} from '@/hooks/use-online-game';
import type { RoundRecord, PendingColumnClear } from '@/hooks/use-interactive-game';
import type { InteractiveGameState, PlayerAction, ActionNeeded } from '@/types';

// --- API helpers ---

async function createRoom(playerName: string, numPlayers: number, rules: string) {
  const res = await fetch('/api/rooms', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_name: playerName, num_players: numPlayers, rules }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<{ room_code: string; session_token: string; player_index: number }>;
}

async function joinRoom(code: string, playerName: string) {
  const res = await fetch(`/api/rooms/${code}/join`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player_name: playerName }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<{ session_token: string; player_index: number }>;
}

// --- Main Route ---

export default function PlayOnlineRoute() {
  const { roomCode: urlRoomCode } = useParams<{ roomCode?: string }>();
  const navigate = useNavigate();
  const { backendAvailable } = useAuth();

  const [sessionToken, setSessionToken] = useState<string | null>(() =>
    sessionStorage.getItem('skyjo-online-token')
  );
  const [roomCode, setRoomCode] = useState<string | null>(() =>
    urlRoomCode || sessionStorage.getItem('skyjo-online-room')
  );
  const [playerIndex, setPlayerIndex] = useState<number | null>(() => {
    const stored = sessionStorage.getItem('skyjo-online-player');
    return stored ? parseInt(stored, 10) : null;
  });

  const game = useOnlineGame(roomCode, sessionToken, playerIndex);

  // Persist session info
  useEffect(() => {
    if (sessionToken && roomCode && playerIndex != null) {
      sessionStorage.setItem('skyjo-online-token', sessionToken);
      sessionStorage.setItem('skyjo-online-room', roomCode);
      sessionStorage.setItem('skyjo-online-player', playerIndex.toString());
    }
  }, [sessionToken, roomCode, playerIndex]);

  // Update URL when room code changes
  useEffect(() => {
    if (roomCode && !urlRoomCode) {
      navigate(`/play/online/${roomCode}`, { replace: true });
    }
  }, [roomCode, urlRoomCode, navigate]);

  const [formError, setFormError] = useState<string | null>(null);

  // Validate room exists when landing on a URL with a room code but no session
  useEffect(() => {
    if (urlRoomCode && !sessionToken) {
      fetch(`/api/rooms/${urlRoomCode}`)
        .then(res => {
          if (!res.ok) {
            setFormError(`Room "${urlRoomCode}" does not exist.`);
            navigate('/play/online', { replace: true });
          }
        })
        .catch(() => {
          setFormError('Could not reach the server. Are you online?');
          navigate('/play/online', { replace: true });
        });
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleCreate = useCallback(async (playerName: string, numPlayers: number, rules: string) => {
    setFormError(null);
    try {
      const result = await createRoom(playerName, numPlayers, rules);
      sessionStorage.setItem('skyjo-online-name', playerName);
      setRoomCode(result.room_code);
      setSessionToken(result.session_token);
      setPlayerIndex(result.player_index);
    } catch (e) {
      setFormError(e instanceof Error ? e.message : 'Failed to create room');
    }
  }, []);

  const handleJoin = useCallback(async (code: string, playerName: string) => {
    setFormError(null);
    try {
      const result = await joinRoom(code, playerName);
      sessionStorage.setItem('skyjo-online-name', playerName);
      setRoomCode(code);
      setSessionToken(result.session_token);
      setPlayerIndex(result.player_index);
    } catch (e) {
      setFormError(e instanceof Error ? e.message : 'Failed to join room');
    }
  }, []);

  const handleLeave = useCallback(() => {
    game.disconnect();
    sessionStorage.removeItem('skyjo-online-token');
    sessionStorage.removeItem('skyjo-online-room');
    sessionStorage.removeItem('skyjo-online-player');
    setRoomCode(null);
    setSessionToken(null);
    setPlayerIndex(null);
    navigate('/play/online', { replace: true });
  }, [game, navigate]);

  // Kicked
  if (game.kicked) {
    return (
      <div className="max-w-md mx-auto space-y-4 text-center">
        <h1 className="text-2xl font-bold">Kicked from Room</h1>
        <p className="text-muted-foreground">{game.lastError || 'You were removed from the room by the host.'}</p>
        <Button onClick={handleLeave}>Back to Online Play</Button>
      </div>
    );
  }

  // Not connected to a room yet — show create/join
  if (!backendAvailable) {
    return (
      <div className="max-w-md mx-auto space-y-6">
        <h1 className="text-2xl font-bold text-center">Online Play</h1>
        <div className="text-center py-8 text-muted-foreground">
          <p className="text-lg font-medium mb-2">Server unavailable</p>
          <p>Online play requires a connection to the game server.</p>
          <p className="mt-3">
            <Link to="/play" className="underline">Play locally</Link> for offline mode.
          </p>
        </div>
      </div>
    );
  }

  if (!roomCode || !sessionToken) {
    return (
      <JoinOrCreate
        initialRoomCode={urlRoomCode || ''}
        onCreate={handleCreate}
        onJoin={handleJoin}
        error={formError}
      />
    );
  }

  return (
    <div className="space-y-4">
      <ConnectionIndicator status={game.connectionStatus} roomCode={roomCode} />

      {game.lastError && (
        <div className="bg-destructive/15 text-destructive px-4 py-2 rounded-md text-sm">
          {game.lastError}
        </div>
      )}

      {game.roomState && !game.gameState && (
        <Lobby
          state={game.roomState}
          playerIndex={playerIndex!}
          onConfigureSlot={game.configureSlot}
          onSetNumPlayers={game.setNumPlayers}
          onSetRules={game.setRules}
          onSetTurnTimer={game.setTurnTimer}
          onKickPlayer={game.kickPlayer}
          onBanPlayer={game.banPlayer}
          onPromoteHost={game.promoteHost}
          onStartGame={game.startGame}
          onLeave={handleLeave}
        />
      )}

      {game.gameState && (
        <OnlinePlayBoard
          state={game.gameState}
          playerIndex={playerIndex!}
          turnDeadlineSecs={game.turnDeadlineSecs}
          wasTimeout={game.wasTimeout}
          onAction={game.applyAction}
          onContinueRound={game.continueRound}
          onPlayAgain={game.playAgain}
          onReturnToLobby={game.returnToLobby}
          onLeave={handleLeave}
          pendingClearColumns={game.pendingClearColumns}
        />
      )}
    </div>
  );
}

// --- Join or Create ---

function JoinOrCreate({
  initialRoomCode,
  onCreate,
  onJoin,
  error,
}: {
  initialRoomCode: string;
  onCreate: (name: string, numPlayers: number, rules: string) => void;
  onJoin: (code: string, name: string) => void;
  error: string | null;
}) {
  const [mode, setMode] = useState<'choose' | 'create' | 'join'>(initialRoomCode ? 'join' : 'choose');
  const { user, isAuthenticated } = useAuth();
  const defaultName = isAuthenticated && user ? user.display_name : '';
  const [name, setName] = useState(() => sessionStorage.getItem('skyjo-online-name') || defaultName);
  const [numPlayers, setNumPlayers] = useState(2);
  const [rules, setRules] = useState('Standard');
  const [joinCode, setJoinCode] = useState(initialRoomCode);

  // For authenticated users: use display_name as default if input is empty
  const effectiveName = name.trim() || (isAuthenticated && user ? user.display_name : '');
  const canSubmit = effectiveName.length > 0;

  return (
    <div className="max-w-md mx-auto space-y-6">
      <h1 className="text-2xl font-bold text-center">Online Play</h1>

      {error && (
        <div className="bg-destructive/15 text-destructive px-4 py-3 rounded-md text-sm text-center">
          {error}
        </div>
      )}

      {mode === 'choose' && (
        <div className="space-y-3">
          <Button onClick={() => setMode('create')} className="w-full" size="lg">
            Create Room
          </Button>
          <Button onClick={() => setMode('join')} variant="outline" className="w-full" size="lg">
            Join Room
          </Button>
        </div>
      )}

      {mode === 'create' && (
        <Card>
          <CardContent className="pt-6 space-y-4">
            <h2 className="text-lg font-semibold">Create Room</h2>
            <div>
              <label className="text-sm font-medium">Your Display Name</label>
              <Input
                value={name}
                onChange={e => setName(e.target.value)}
                placeholder={isAuthenticated && user ? user.display_name : 'Your Name'}
                maxLength={20}
              />
              <p className="text-xs text-muted-foreground mt-1">
                This is your player name, visible to everyone in the room.
              </p>
            </div>
            <div>
              <label className="text-sm font-medium">Number of Players</label>
              <Select value={numPlayers.toString()} onValueChange={v => setNumPlayers(parseInt(v))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {[2, 3, 4, 5, 6, 7, 8].map(n => (
                    <SelectItem key={n} value={n.toString()}>{n} players</SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground mt-1">
                You can change this in the lobby and fill empty slots with bots.
              </p>
            </div>
            <div>
              <label className="text-sm font-medium">Rules</label>
              <Select value={rules} onValueChange={setRules}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="Standard">Standard</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex gap-2">
              <Button onClick={() => setMode('choose')} variant="outline">Back</Button>
              <Button
                onClick={() => onCreate(effectiveName, numPlayers, rules)}
                className="flex-1"
                disabled={!canSubmit}
              >
                Create
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {mode === 'join' && (
        <Card>
          <CardContent className="pt-6 space-y-4">
            <h2 className="text-lg font-semibold">Join Room</h2>
            <div>
              <label className="text-sm font-medium">Your Display Name</label>
              <Input
                value={name}
                onChange={e => setName(e.target.value)}
                placeholder={isAuthenticated && user ? user.display_name : 'Your Name'}
                maxLength={20}
              />
            </div>
            <div>
              <label className="text-sm font-medium">Room Code</label>
              <RoomCodeInput value={joinCode} onChange={setJoinCode} />
            </div>
            <div className="flex gap-2">
              <Button onClick={() => setMode('choose')} variant="outline">Back</Button>
              <Button
                onClick={() => onJoin(joinCode, effectiveName)}
                className="flex-1"
                disabled={!canSubmit || joinCode.replace(/[^A-Z0-9]/gi, '').length < 6}
              >
                Join
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      <p className="text-center text-sm text-muted-foreground">
        Online play requires an internet connection.{' '}
        <a href="/play" className="underline">Play locally</a> for offline mode.
      </p>
    </div>
  );
}

// --- OTP-style Room Code Input ---

function RoomCodeInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const CODE_LENGTH = 6;
  // Always produce exactly CODE_LENGTH entries — never shrink the array
  const chars = Array.from({ length: CODE_LENGTH }, (_, i) => value[i] ?? '');
  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);

  const buildValue = (arr: string[]) =>
    arr.join('').replace(/\s/g, '');

  const handleChange = (index: number, char: string) => {
    const upper = char.toUpperCase().replace(/[^A-Z0-9]/g, '');
    if (!upper) return;

    const newChars = [...chars];
    newChars[index] = upper[0];
    onChange(buildValue(newChars));

    if (index < CODE_LENGTH - 1) {
      inputRefs.current[index + 1]?.focus();
    }
  };

  const handleKeyDown = (index: number, e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Backspace') {
      e.preventDefault();
      if (value.length > 0) {
        // Remove the last character and focus the appropriate cell
        const newValue = value.slice(0, -1);
        onChange(newValue);
        const focusIdx = Math.max(0, newValue.length - 1);
        // Use setTimeout so the re-render completes before focusing
        setTimeout(() => inputRefs.current[focusIdx]?.focus(), 0);
      }
    } else if (e.key === 'ArrowLeft' && index > 0) {
      inputRefs.current[index - 1]?.focus();
    } else if (e.key === 'ArrowRight' && index < CODE_LENGTH - 1) {
      inputRefs.current[index + 1]?.focus();
    }
  };

  const handlePaste = (e: React.ClipboardEvent) => {
    e.preventDefault();
    const pasted = e.clipboardData.getData('text').toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, CODE_LENGTH);
    if (pasted) {
      onChange(pasted);
      const focusIdx = Math.min(pasted.length, CODE_LENGTH - 1);
      inputRefs.current[focusIdx]?.focus();
    }
  };

  return (
    <div className="flex gap-1.5 justify-center">
      {chars.map((char, i) => (
        <input
          key={i}
          ref={el => { inputRefs.current[i] = el; }}
          type="text"
          inputMode="text"
          autoComplete="one-time-code"
          maxLength={1}
          value={char}
          onChange={e => handleChange(i, e.target.value)}
          onKeyDown={e => handleKeyDown(i, e)}
          onPaste={handlePaste}
          onFocus={e => e.target.select()}
          className={cn(
            'w-9 h-11 sm:w-10 sm:h-12 text-center text-lg sm:text-xl font-mono font-bold rounded-md border border-input bg-background',
            'focus:outline-none focus:ring-2 focus:ring-ring focus:border-transparent',
            'uppercase transition-colors',
          )}
        />
      ))}
    </div>
  );
}

// --- Room Timer ---

function RoomTimer({ initialSecs }: { initialSecs: number }) {
  const [secs, setSecs] = useState(initialSecs);

  useEffect(() => {
    setSecs(initialSecs);
  }, [initialSecs]);

  useEffect(() => {
    if (secs <= 0) return;
    const id = setInterval(() => setSecs(s => Math.max(0, s - 1)), 1000);
    return () => clearInterval(id);
  }, [secs > 0]); // eslint-disable-line react-hooks/exhaustive-deps

  const mins = Math.floor(secs / 60);
  const seconds = secs % 60;
  const isLow = secs < 5 * 60;

  return (
    <p className={cn(
      'text-xs mt-1',
      isLow ? 'text-destructive' : 'text-muted-foreground',
    )}>
      Room expires in {mins}:{seconds.toString().padStart(2, '0')}
    </p>
  );
}

// --- Connection Indicator ---

function ConnectionIndicator({ status, roomCode }: { status: ConnectionStatus; roomCode: string }) {
  return (
    <div className="flex items-center gap-2 text-sm">
      <div className={cn(
        'w-2 h-2 rounded-full',
        status === 'connected' && 'bg-green-500',
        status === 'connecting' && 'bg-yellow-500 animate-pulse',
        status === 'disconnected' && 'bg-red-500',
      )} />
      <span className="text-muted-foreground">
        {status === 'connected' && 'Connected'}
        {status === 'connecting' && 'Connecting...'}
        {status === 'disconnected' && 'Disconnected'}
      </span>
      <Badge variant="outline" className="font-mono ml-auto">
        {roomCode}
      </Badge>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => navigator.clipboard.writeText(roomCode)}
      >
        Copy Code
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => navigator.clipboard.writeText(`${window.location.origin}/play/online/${roomCode}`)}
      >
        Copy Link
      </Button>
    </div>
  );
}

// --- Lobby ---

const TURN_TIMER_OPTIONS: { value: string; label: string }[] = [
  { value: '30', label: '30s' },
  { value: '60', label: '60s' },
  { value: '90', label: '90s' },
  { value: '120', label: '120s' },
  { value: 'unlimited', label: 'Unlimited' },
];

function Lobby({
  state,
  playerIndex,
  onConfigureSlot,
  onSetNumPlayers,
  onSetRules,
  onSetTurnTimer,
  onKickPlayer,
  onBanPlayer,
  onPromoteHost,
  onStartGame,
  onLeave,
}: {
  state: RoomLobbyState;
  playerIndex: number;
  onConfigureSlot: (slot: number, playerType: string) => void;
  onSetNumPlayers: (n: number) => void;
  onSetRules: (rules: string) => void;
  onSetTurnTimer: (secs: number | null) => void;
  onKickPlayer: (slot: number) => void;
  onBanPlayer: (slot: number) => void;
  onPromoteHost: (slot: number) => void;
  onStartGame: () => void;
  onLeave: () => void;
}) {
  const isCreator = playerIndex === state.creator;
  const hasEmptySlots = state.players.some(p => p.player_type.kind === 'Empty');
  const hasDisconnectedHuman = state.players.some(
    p => p.player_type.kind === 'Human' && !p.connected
  );
  const cantStart = hasEmptySlots || hasDisconnectedHuman;
  const [banConfirmSlot, setBanConfirmSlot] = useState<number | null>(null);

  return (
    <Card>
      <CardContent className="pt-6 space-y-4">
        <div className="text-center">
          <h2 className="text-xl font-bold">Room {state.room_code}</h2>
          <p className="text-sm text-muted-foreground">
            {state.rules} rules
          </p>
          {state.idle_timeout_secs != null && (
            <RoomTimer initialSecs={state.idle_timeout_secs} />
          )}
        </div>

        {/* Room config (creator only) */}
        {isCreator && (
          <div className="flex items-center gap-4 justify-center flex-wrap">
            <div className="flex items-center gap-2">
              <label className="text-sm font-medium">Players:</label>
              <Select
                value={state.num_players.toString()}
                onValueChange={v => onSetNumPlayers(parseInt(v))}
              >
                <SelectTrigger className="w-20 h-8 text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {[2, 3, 4, 5, 6, 7, 8].map(n => (
                    <SelectItem key={n} value={n.toString()}>{n}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-center gap-2">
              <label className="text-sm font-medium">Rules:</label>
              <Select
                value={state.rules}
                onValueChange={onSetRules}
              >
                <SelectTrigger className="w-28 h-8 text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {state.available_rules.map(r => (
                    <SelectItem key={r} value={r}>{r}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-center gap-2">
              <label className="text-sm font-medium">Turn Timer:</label>
              <Select
                value={state.turn_timer_secs?.toString() ?? 'unlimited'}
                onValueChange={v => onSetTurnTimer(v === 'unlimited' ? null : parseInt(v))}
              >
                <SelectTrigger className="w-28 h-8 text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {TURN_TIMER_OPTIONS.map(opt => (
                    <SelectItem key={opt.value} value={opt.value}>{opt.label}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        )}

        {/* Show turn timer info for non-creators */}
        {!isCreator && (
          <p className="text-xs text-muted-foreground text-center">
            Turn timer: {state.turn_timer_secs ? `${state.turn_timer_secs}s` : 'Unlimited'}
          </p>
        )}

        <div className="space-y-2">
          <h3 className="font-medium text-sm">Players</h3>
          {state.players.map((player, i) => (
            <div
              key={i}
              className={cn(
                'flex items-center gap-2 px-3 py-2 rounded-md border',
                i === playerIndex && 'border-primary bg-primary/5',
              )}
            >
              <div className={cn(
                'w-2 h-2 rounded-full shrink-0',
                player.player_type.kind === 'Bot' ? 'bg-blue-500' :
                player.player_type.kind === 'Empty' ? 'bg-gray-300' :
                player.connected ? 'bg-green-500' : 'bg-red-500',
              )} />

              <span className="flex-1 text-sm min-w-0 flex items-center gap-1 flex-wrap">
                {player.player_type.kind === 'Empty' ? (
                  <span className="text-muted-foreground italic">Waiting for player...</span>
                ) : (
                  <>
                    {state.last_winners.includes(i) && (
                      <span className="text-yellow-500" title="Winner of last game">👑</span>
                    )}
                    <span className="truncate">{player.name}</span>
                    {i === state.creator && (
                      <Badge className="bg-amber-500 text-white text-[10px] py-0 px-1">host</Badge>
                    )}
                    {i === playerIndex && (
                      <Badge variant="outline" className="text-[10px] py-0 px-1">you</Badge>
                    )}
                    {player.disconnect_secs != null && (
                      <span className="text-xs text-destructive">
                        (dc {Math.max(0, 60 - player.disconnect_secs)}s)
                      </span>
                    )}
                  </>
                )}
              </span>

              {/* Bot/empty slot configuration (creator only, not for self or other humans) */}
              {isCreator && i !== playerIndex && player.player_type.kind !== 'Human' && (
                <Select
                  value={
                    player.player_type.kind === 'Bot'
                      ? `Bot:${player.player_type.strategy}`
                      : 'Empty'
                  }
                  onValueChange={v => onConfigureSlot(i, v)}
                >
                  <SelectTrigger className="w-32 h-8 text-xs shrink-0">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="Empty">Empty</SelectItem>
                    {state.available_strategies.map(s => (
                      <SelectItem key={s} value={`Bot:${s}`}>
                        Bot: {s}{s === 'Genetic' && state.genetic_generation > 0 ? ` (Gen ${state.genetic_generation})` : ''}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}

              {/* Non-creator sees bot badges */}
              {!isCreator && player.player_type.kind === 'Bot' && (
                <Badge variant="secondary" className="text-xs shrink-0">
                  Bot: {player.player_type.strategy}{player.player_type.strategy === 'Genetic' && state.genetic_generation > 0 ? ` (Gen ${state.genetic_generation})` : ''}
                </Badge>
              )}

              {/* Host actions for other humans */}
              {isCreator && i !== playerIndex && player.player_type.kind === 'Human' && (
                <div className="flex gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    onClick={() => onPromoteHost(i)}
                    title="Make this player the host"
                  >
                    Promote
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                    onClick={() => onKickPlayer(i)}
                  >
                    Kick
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                    onClick={() => setBanConfirmSlot(i)}
                  >
                    Ban
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>

        <div className="flex gap-2">
          <Button variant="outline" onClick={onLeave}>Leave</Button>
          {isCreator && (
            <Button
              className="flex-1"
              onClick={onStartGame}
              disabled={cantStart}
              title={
                hasEmptySlots ? 'Fill empty slots with bots or wait for players' :
                hasDisconnectedHuman ? 'A player is disconnected — wait for them or kick/replace' :
                undefined
              }
            >
              {hasEmptySlots ? 'Fill all slots to start' :
               hasDisconnectedHuman ? 'Waiting for players to connect' :
               'Start Game'}
            </Button>
          )}
          {!isCreator && (
            <p className="flex-1 text-center text-sm text-muted-foreground self-center">
              Waiting for host to start...
            </p>
          )}
        </div>

        {isCreator && hasEmptySlots && (
          <p className="text-xs text-muted-foreground text-center">
            Tip: Use the dropdowns to fill empty slots with bots, or reduce the player count above.
          </p>
        )}
      </CardContent>

      {/* Ban confirmation dialog */}
      <Dialog open={banConfirmSlot !== null} onOpenChange={open => { if (!open) setBanConfirmSlot(null); }}>
        <DialogContent>
          {banConfirmSlot !== null && (() => {
            const target = state.players[banConfirmSlot];
            const sharesIp = target?.shares_ip_with_host === true;
            return (
              <>
                <DialogHeader>
                  <DialogTitle>Ban player?</DialogTitle>
                  <DialogDescription>
                    <strong>{target?.name}</strong> will be removed and won't be able to rejoin this room.
                  </DialogDescription>
                </DialogHeader>
                {sharesIp ? (
                  <div className="bg-destructive/15 text-destructive px-4 py-3 rounded-md text-sm">
                    This player is on the same network as you. Banning them would also ban you from your own room.
                  </div>
                ) : (
                  <p className="text-sm text-amber-600 dark:text-amber-400">
                    IP bans affect all users sharing the same network (e.g., same household or office Wi-Fi). Use this only for genuinely disruptive players.
                  </p>
                )}
                <div className="flex gap-2 justify-end">
                  <Button variant="outline" onClick={() => setBanConfirmSlot(null)}>
                    Cancel
                  </Button>
                  <Button
                    variant="destructive"
                    disabled={sharesIp}
                    onClick={() => {
                      onBanPlayer(banConfirmSlot);
                      setBanConfirmSlot(null);
                    }}
                  >
                    Ban Player
                  </Button>
                </div>
              </>
            );
          })()}
        </DialogContent>
      </Dialog>
    </Card>
  );
}

// --- Online Game Board ---

function TurnTimer({ deadlineSecs }: { deadlineSecs: number }) {
  const [secs, setSecs] = useState(deadlineSecs);

  useEffect(() => {
    setSecs(deadlineSecs);
  }, [deadlineSecs]);

  useEffect(() => {
    if (secs <= 0) return;
    const id = setInterval(() => setSecs(s => Math.max(0, s - 1)), 1000);
    return () => clearInterval(id);
  }, [secs > 0]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <span className={cn(
      'text-sm font-mono font-bold',
      secs > 15 ? 'text-muted-foreground' :
      secs > 5 ? 'text-yellow-600 dark:text-yellow-400' :
      'text-red-600 dark:text-red-400 animate-pulse',
    )}>
      {secs}s
    </span>
  );
}

function OnlinePlayBoard({
  state,
  playerIndex,
  turnDeadlineSecs,
  wasTimeout,
  onAction,
  onContinueRound,
  onPlayAgain,
  onReturnToLobby,
  onLeave,
  pendingClearColumns,
}: {
  state: InteractiveGameState;
  playerIndex: number;
  turnDeadlineSecs: number | null;
  wasTimeout: boolean;
  onAction: (action: PlayerAction) => void;
  onContinueRound: () => void;
  onPlayAgain: () => void;
  onReturnToLobby: () => void;
  onLeave: () => void;
  pendingClearColumns: PendingColumnClear[] | null;
}) {
  const { action_needed, boards, num_rows, num_cols, current_player } = state;
  const [wantsFlip, setWantsFlip] = useState(false);
  const cardSizes = useResponsiveCardSize();
  const activeBoardRef = useRef<HTMLDivElement>(null);

  // Track round history for the scorecard
  const [roundHistory, setRoundHistory] = useState<RoundRecord[]>([]);
  const recordedRoundsRef = useRef(new Set<number>());

  // Record round results when RoundOver is shown
  useEffect(() => {
    if (action_needed.type === 'RoundOver' && !recordedRoundsRef.current.has(action_needed.round_number)) {
      recordedRoundsRef.current.add(action_needed.round_number);
      setRoundHistory(prev => [...prev, {
        roundNumber: action_needed.round_number,
        roundScores: action_needed.round_scores,
        rawRoundScores: action_needed.raw_round_scores,
        cumulativeScores: action_needed.cumulative_scores,
        goingOutPlayer: action_needed.going_out_player,
      }]);
    }
  }, [action_needed]);

  // Round / game over screens
  if (action_needed.type === 'RoundOver') {
    return (
      <OnlineRoundSummary
        state={state}
        actionNeeded={action_needed}
        onContinue={onContinueRound}
      />
    );
  }

  if (action_needed.type === 'GameOver') {
    return (
      <OnlineGameOver
        state={state}
        actionNeeded={action_needed}
        onPlayAgain={onPlayAgain}
        onReturnToLobby={onReturnToLobby}
        onLeave={onLeave}
      />
    );
  }

  const activePlayer = action_needed.type === 'ChooseInitialFlips'
    ? action_needed.player
    : current_player;

  const isMyTurn = activePlayer === playerIndex;
  const isInitialFlips = action_needed.type === 'ChooseInitialFlips';
  const isChooseDraw = action_needed.type === 'ChooseDraw';
  const isDeckDrawAction = action_needed.type === 'ChooseDeckDrawAction';
  const isDiscardPlacement = action_needed.type === 'ChooseDiscardDrawPlacement';

  // Whether the game is in a draw/play phase (for stable layout)
  const isPlayPhase = action_needed.type === 'ChooseDraw'
    || action_needed.type === 'ChooseDeckDrawAction'
    || action_needed.type === 'ChooseDiscardDrawPlacement';
  const hasDrawnCard = (isDeckDrawAction && action_needed.drawn_card != null)
    || isDiscardPlacement;

  // Prompt text
  let prompt: string;
  if (!isMyTurn) {
    prompt = `Waiting for ${getPlayerName(state, activePlayer)}...`;
  } else if (isInitialFlips) {
    const remaining = action_needed.count;
    prompt = `Click ${remaining} hidden card${remaining !== 1 ? 's' : ''} to flip`;
  } else if (isChooseDraw) {
    prompt = 'Draw from the deck or discard pile';
  } else if (isDeckDrawAction) {
    const card = action_needed.drawn_card;
    prompt = card != null
      ? (wantsFlip
        ? `Click a hidden card to flip (discarding the ${card})`
        : `Click a card to replace with your ${card}, or discard & flip instead`)
      : 'Waiting...';
  } else if (isDiscardPlacement) {
    const card = action_needed.drawn_card;
    prompt = `Click a card to replace with your ${card}`;
  } else {
    prompt = '';
  }

  // Card interactivity
  const getCardInteractive = (boardPlayerIdx: number, pos: number): boolean => {
    if (!isMyTurn) return false;
    if (isInitialFlips) {
      if (boardPlayerIdx !== playerIndex) return false;
      return boards[boardPlayerIdx][pos] === 'Hidden';
    }
    if (boardPlayerIdx !== current_player) return false;
    if (isDeckDrawAction) {
      if (wantsFlip) return boards[boardPlayerIdx][pos] === 'Hidden';
      return boards[boardPlayerIdx][pos] !== 'Cleared';
    }
    if (isDiscardPlacement) return boards[boardPlayerIdx][pos] !== 'Cleared';
    return false;
  };

  const handleCardClick = (boardPlayerIdx: number, position: number) => {
    if (!isMyTurn) return;
    if (isInitialFlips && boardPlayerIdx === playerIndex) {
      if (boards[boardPlayerIdx][position] === 'Hidden') {
        onAction({ type: 'InitialFlip', position });
      }
    } else if (isDeckDrawAction) {
      if (wantsFlip) {
        if (boards[boardPlayerIdx][position] === 'Hidden') {
          onAction({ type: 'DiscardAndFlip', position });
          setWantsFlip(false);
        }
      } else {
        if (boards[boardPlayerIdx][position] !== 'Cleared') {
          onAction({ type: 'KeepDeckDraw', position });
        }
      }
    } else if (isDiscardPlacement) {
      if (boards[boardPlayerIdx][position] !== 'Cleared') {
        onAction({ type: 'PlaceDiscardDraw', position });
      }
    }
  };

  const handleDrawDeck = () => {
    if (isMyTurn && isChooseDraw) {
      onAction({ type: 'DrawFromDeck' });
    }
  };

  const handleDrawDiscard = (pileIndex: number) => {
    if (isMyTurn && isChooseDraw) {
      onAction({ type: 'DrawFromDiscard', pile_index: pileIndex });
    }
  };

  const hasGoneOut = state.going_out_player !== null;

  // Auto-scroll to active player's board on mobile
  // eslint-disable-next-line react-hooks/rules-of-hooks
  useEffect(() => {
    if (window.innerWidth < 640 && activeBoardRef.current) {
      activeBoardRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [activePlayer]);

  return (
    <div className="space-y-4">
      {/* Info / going-out banner */}
      <div
        className={cn(
          'rounded-lg border p-3 text-center transition-all duration-300',
          hasGoneOut
            ? 'border-orange-400 border-2 bg-orange-50 dark:bg-orange-950/30'
            : 'border-border bg-muted/30'
        )}
      >
        {hasGoneOut ? (
          <>
            <p className="text-sm font-bold text-orange-600 dark:text-orange-400">
              {getPlayerName(state, state.going_out_player!)} has gone out!
            </p>
            <p className="text-xs text-orange-500 dark:text-orange-400/80 mt-0.5">
              Each remaining player gets one final turn.
            </p>
          </>
        ) : (
          <>
            <p className="text-sm font-medium text-muted-foreground">
              Online Game · {state.num_players} players
            </p>
            <p className="text-xs text-muted-foreground/70 mt-0.5">
              Round {state.round_number + 1} · {state.deck_remaining} cards in deck
            </p>
          </>
        )}
      </div>

      {/* Timeout flash */}
      {wasTimeout && (
        <div className="text-center text-sm font-medium text-red-600 dark:text-red-400 animate-pulse">
          Time ran out! A random move was played.
        </div>
      )}

      {/* Status bar */}
      <div className="text-center space-y-1">
        <h3 className={cn(
          'text-lg font-semibold flex items-center justify-center gap-2',
          state.is_final_turn && 'text-orange-600 dark:text-orange-400'
        )}>
          <span>
            {getPlayerName(state, activePlayer)}'s{' '}
            {state.is_final_turn ? 'Final Turn!' : 'Turn'}
          </span>
          {turnDeadlineSecs != null && turnDeadlineSecs > 0 && (
            <TurnTimer deadlineSecs={turnDeadlineSecs} />
          )}
        </h3>
        <p className="text-sm text-muted-foreground">
          Round {state.round_number + 1}
        </p>
        <p className="text-sm font-medium text-primary">{prompt}</p>
      </div>

      {/* Draw area — always rendered during play phase for layout stability */}
      {isPlayPhase && (
        <div className="flex flex-wrap items-center justify-center gap-2 sm:gap-4 md:gap-8">
          {/* Deck + Discard piles group */}
          <div className="flex items-center gap-2 sm:gap-4 md:gap-8">
            {/* Deck */}
            <button
              onClick={handleDrawDeck}
              disabled={!isMyTurn || !isChooseDraw}
              className={cn(
                'flex flex-col items-center gap-1 transition-transform',
                isMyTurn && isChooseDraw && 'hover:scale-105 cursor-pointer'
              )}
            >
              <span className="text-xs text-muted-foreground">
                Deck ({state.deck_remaining})
              </span>
              <div
                className={cn(
                  'rounded-lg',
                  isMyTurn && isChooseDraw && 'ring-2 ring-blue-400'
                )}
              >
                <SkyjoCard slot={{ Hidden: 0 }} size={cardSizes.draw} />
              </div>
            </button>

            {/* Discard piles */}
            {state.discard_tops.map((top, pileIdx) => (
              <button
                key={pileIdx}
                onClick={() => handleDrawDiscard(pileIdx)}
                disabled={
                  !isMyTurn ||
                  !isChooseDraw ||
                  top === null ||
                  !action_needed.drawable_piles?.includes(pileIdx)
                }
                className={cn(
                  'flex flex-col items-center gap-1 transition-transform',
                  isMyTurn &&
                    isChooseDraw &&
                    top !== null &&
                    action_needed.drawable_piles?.includes(pileIdx) &&
                    'hover:scale-105 cursor-pointer'
                )}
              >
                <span className="text-xs text-muted-foreground">
                  Discard ({state.discard_sizes[pileIdx]})
                </span>
                <div
                  className={cn(
                    'rounded-lg',
                    isMyTurn &&
                      isChooseDraw &&
                      top !== null &&
                      action_needed.drawable_piles?.includes(pileIdx) &&
                      'ring-2 ring-blue-400'
                  )}
                >
                  {top !== null ? (
                    <SkyjoCard slot={{ Revealed: top }} size={cardSizes.draw} />
                  ) : (
                    <SkyjoCard slot="Cleared" size={cardSizes.draw} />
                  )}
                </div>
              </button>
            ))}
          </div>

          {/* Drawn card + Action buttons group */}
          <div className="flex items-center gap-2 sm:gap-4 md:gap-8">
            {/* Drawn card / placeholder — stable slot */}
            <div className="flex flex-col items-center gap-1">
              <span className="text-xs text-muted-foreground">Drawn</span>
              {hasDrawnCard ? (
                <div className="ring-2 ring-green-400 rounded-lg">
                  <SkyjoCard
                    slot={{ Revealed: action_needed.drawn_card! }}
                    size={cardSizes.draw}
                  />
                </div>
              ) : (
                <SkyjoCard slot="Cleared" size={cardSizes.draw} />
              )}
            </div>

            {/* Action icon buttons */}
            <ActionButtons
              wantsFlip={wantsFlip}
              onToggleFlip={() => setWantsFlip(!wantsFlip)}
              onUndo={() => onAction({ type: 'UndoDrawFromDiscard' })}
              trashEnabled={isMyTurn && isDeckDrawAction}
              undoEnabled={isMyTurn && isDiscardPlacement}
            />
          </div>
        </div>
      )}

      {/* Column clear notification */}
      {state.last_column_clears.length > 0 && (
        <div className="text-center text-sm font-medium text-green-600">
          Column cleared! ({state.last_column_clears.map(c => {
            const displaced = c.displaced_card !== null ? `, discarded ${c.displaced_card}` : '';
            return `column ${c.column + 1}${displaced}`;
          }).join('; ')})
        </div>
      )}

      {/* Player boards */}
      <div className="flex flex-wrap gap-2 sm:gap-4 justify-center">
        {boards.map((board, boardPlayerIdx) => {
          const isActive = boardPlayerIdx === activePlayer;
          const cardSize = isActive ? cardSizes.boardActive : cardSizes.board;

          return (
            <div
              key={boardPlayerIdx}
              ref={isActive ? activeBoardRef : undefined}
              className={cn(
                'rounded-lg border p-3 transition-colors',
                isActive && !state.is_final_turn && 'border-blue-500 border-2',
                isActive && state.is_final_turn && 'border-orange-500 border-2 bg-orange-50/50 dark:bg-orange-950/20',
                !isActive && boardPlayerIdx === state.going_out_player && 'border-orange-300 border-2 opacity-75',
                !isActive && boardPlayerIdx !== state.going_out_player && 'border-border',
                boardPlayerIdx === playerIndex && 'bg-primary/5',
              )}
            >
              <h4 className="text-sm font-medium mb-2 flex items-center gap-1">
                {getPlayerName(state, boardPlayerIdx)}
                {boardPlayerIdx === playerIndex && (
                  <Badge variant="outline" className="text-[10px] py-0">you</Badge>
                )}
                {boardPlayerIdx === state.going_out_player && (
                  <span className="text-xs font-semibold text-orange-500 ml-1">went out</span>
                )}
              </h4>
              <div className="flex gap-0.5 sm:gap-1">
                {Array.from({ length: num_cols }, (_, c) => {
                  const clearInfo = pendingClearColumns?.find(
                    pc => pc.playerIndex === boardPlayerIdx && pc.column === c
                  );
                  const isColumnClearing = !!clearInfo;

                  let clearStyle: React.CSSProperties | undefined;
                  if (isColumnClearing) {
                    const firstIdx = c * num_rows;
                    const slot = board[firstIdx];
                    const val = typeof slot === 'object' && 'Revealed' in slot ? slot.Revealed : 0;
                    const colors = COLUMN_CLEAR_COLORS[getCardColorGroup(val as Parameters<typeof getCardColorGroup>[0])];
                    clearStyle = {
                      '--clear-color-base': colors.base,
                      '--clear-color-bright': colors.bright,
                      '--clear-color-glow': colors.glow,
                    } as React.CSSProperties;
                  }

                  return (
                    <div
                      key={c}
                      className={cn(
                        'flex flex-col gap-0.5 sm:gap-1 rounded-lg transition-all duration-300',
                        isColumnClearing && 'outline-3 outline animate-[border-pulse_1.5s_ease-in-out_infinite]',
                      )}
                      style={clearStyle}
                    >
                      {Array.from({ length: num_rows }, (_, r) => {
                        const idx = c * num_rows + r;
                        const interactive = getCardInteractive(boardPlayerIdx, idx);

                        return (
                          <button
                            key={idx}
                            onClick={() => interactive && handleCardClick(boardPlayerIdx, idx)}
                            disabled={!interactive}
                            className={cn(
                              'transition-transform',
                              interactive && 'hover:scale-110 cursor-pointer'
                            )}
                          >
                            <SkyjoCard
                              slot={toSlot(board[idx])}
                              size={cardSize}
                              highlight={interactive}
                            />
                          </button>
                        );
                      })}
                    </div>
                  );
                })}
              </div>
              <div className="text-xs mt-1 space-y-0.5">
                <div className="text-muted-foreground">
                  Visible: {computeVisibleScore(board)}
                </div>
                {state.cumulative_scores[boardPlayerIdx] !== 0 && (
                  <div className="text-muted-foreground">
                    Cumulative: {state.cumulative_scores[boardPlayerIdx]}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* Round Scorecard */}
      <RoundScorecard
        roundHistory={roundHistory}
        playerNames={state.player_names}
        currentCumulativeScores={state.cumulative_scores}
      />
    </div>
  );
}

// --- Round Summary ---

function OnlineRoundSummary({
  state,
  actionNeeded,
  onContinue,
}: {
  state: InteractiveGameState;
  actionNeeded: ActionNeeded & { type: 'RoundOver' };
  onContinue: () => void;
}) {
  const { round_scores, raw_round_scores, cumulative_scores, going_out_player, end_of_round_clears } = actionNeeded;

  return (
    <Card>
      <CardContent className="pt-6 space-y-4">
        <h2 className="text-xl font-bold text-center">
          Round {actionNeeded.round_number + 1} Complete
        </h2>

        {end_of_round_clears.length > 0 && (
          <div className="text-center text-sm text-green-600">
            End-of-round column clears:{' '}
            {end_of_round_clears.map(
              (c) => `${getPlayerName(state, c.player_index)} col ${c.column + 1}`
            ).join(', ')}
          </div>
        )}

        <div className="flex flex-wrap gap-2 sm:gap-4 justify-center">
          {state.boards.map((board, playerIdx) => (
            <div
              key={playerIdx}
              className={cn(
                'rounded-lg border p-3',
                playerIdx === going_out_player && 'border-orange-400 border-2'
              )}
            >
              <h4 className="text-sm font-medium mb-2">
                {getPlayerName(state, playerIdx)}
                {playerIdx === going_out_player && (
                  <span className="text-xs text-orange-500 ml-1">(went out)</span>
                )}
              </h4>
              <div
                className="grid gap-0.5 sm:gap-1"
                style={{ gridTemplateColumns: `repeat(${state.num_cols}, 1fr)` }}
              >
                {Array.from({ length: state.num_rows }, (_, r) =>
                  Array.from({ length: state.num_cols }, (_, c) => {
                    const idx = c * state.num_rows + r;
                    return (
                      <SkyjoCard key={idx} slot={toSlot(board[idx])} size="sm" />
                    );
                  })
                ).flat()}
              </div>
            </div>
          ))}
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b">
                <th className="text-left py-2 pr-4">Player</th>
                <th className="text-center py-2 px-2">Round Score</th>
                <th className="text-center py-2 px-2">Total</th>
              </tr>
            </thead>
            <tbody>
              {state.player_names.map((name, i) => {
                const wasPenalized = round_scores[i] !== raw_round_scores[i];
                return (
                  <tr key={i} className="border-b last:border-0">
                    <td className="py-2 pr-4 font-medium">
                      {name}
                      {i === going_out_player && ' *'}
                    </td>
                    <td className={cn('text-center py-2 px-2', wasPenalized && 'text-destructive')}>
                      {wasPenalized
                        ? <>{raw_round_scores[i]} → <span className="font-bold">{round_scores[i]}</span></>
                        : round_scores[i]
                      }
                    </td>
                    <td className="text-center py-2 px-2 font-bold">{cumulative_scores[i]}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        <Button onClick={onContinue} className="w-full">Next Round</Button>
      </CardContent>
    </Card>
  );
}

// --- Game Over ---

function OnlineGameOver({
  state,
  actionNeeded,
  onPlayAgain,
  onReturnToLobby,
  onLeave,
}: {
  state: InteractiveGameState;
  actionNeeded: ActionNeeded & { type: 'GameOver' };
  onPlayAgain: () => void;
  onReturnToLobby: () => void;
  onLeave: () => void;
}) {
  const { final_scores, winners } = actionNeeded;

  const sorted = [...state.player_names]
    .map((name, i) => ({ name, score: final_scores[i], isWinner: winners.includes(i) }))
    .sort((a, b) => a.score - b.score);

  return (
    <Card>
      <CardContent className="pt-6 space-y-4">
        <h2 className="text-xl font-bold text-center">Game Over</h2>

        <div className="space-y-2">
          {sorted.map((p, i) => (
            <div
              key={i}
              className={cn(
                'flex items-center justify-between px-3 py-2 rounded-md',
                p.isWinner ? 'bg-yellow-100 dark:bg-yellow-900/30 border border-yellow-400' : 'border'
              )}
            >
              <span className="font-medium">
                {i + 1}. {p.name}
                {p.isWinner && ' (Winner!)'}
              </span>
              <span className="font-bold">{p.score}</span>
            </div>
          ))}
        </div>

        <div className="flex gap-2">
          <Button variant="outline" onClick={onLeave}>Leave</Button>
          <Button variant="secondary" className="flex-1" onClick={onReturnToLobby}>Return to Lobby</Button>
          <Button className="flex-1" onClick={onPlayAgain}>Play Again</Button>
        </div>
      </CardContent>
    </Card>
  );
}
