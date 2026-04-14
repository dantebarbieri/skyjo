import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent } from '@/components/ui/card';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import SkyjoCard from '@/components/skyjo-card';
import { useResponsiveCardSize } from '@/hooks/use-responsive-card-size';
import { cn } from '@/lib/utils';
import {
  useOnlineGame,
  type RoomLobbyState,
  type ConnectionStatus,
} from '@/hooks/use-online-game';
import type { InteractiveGameState, PlayerAction, ActionNeeded, VisibleSlot, Slot } from '@/types';

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
          onAction={game.applyAction}
          onContinueRound={game.continueRound}
          onPlayAgain={game.playAgain}
          onReturnToLobby={game.returnToLobby}
          onLeave={handleLeave}
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
  const [name, setName] = useState(() => sessionStorage.getItem('skyjo-online-name') || '');
  const [numPlayers, setNumPlayers] = useState(2);
  const [rules, setRules] = useState('Standard');
  const [joinCode, setJoinCode] = useState(initialRoomCode);

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
                placeholder="How others will see you"
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
                onClick={() => onCreate(name.trim() || 'Player 1', numPlayers, rules)}
                className="flex-1"
                disabled={!name.trim()}
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
                placeholder="How others will see you"
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
                onClick={() => onJoin(joinCode, name.trim() || 'Player')}
                className="flex-1"
                disabled={!name.trim() || joinCode.replace(/[^A-Z0-9]/gi, '').length < 6}
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

function Lobby({
  state,
  playerIndex,
  onConfigureSlot,
  onSetNumPlayers,
  onSetRules,
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
          </div>
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

/** Convert a VisibleSlot to a Slot for SkyjoCard rendering */
function toSlot(vs: VisibleSlot): Slot {
  if (vs === 'Hidden') return { Hidden: 0 };
  if (vs === 'Cleared') return 'Cleared';
  return { Revealed: vs.Revealed };
}

function getPlayerName(state: InteractiveGameState, index: number) {
  return state.player_names[index] || `Player ${index + 1}`;
}

function OnlinePlayBoard({
  state,
  playerIndex,
  onAction,
  onContinueRound,
  onPlayAgain,
  onReturnToLobby,
  onLeave,
}: {
  state: InteractiveGameState;
  playerIndex: number;
  onAction: (action: PlayerAction) => void;
  onContinueRound: () => void;
  onPlayAgain: () => void;
  onReturnToLobby: () => void;
  onLeave: () => void;
}) {
  const { action_needed, boards, num_rows, num_cols, current_player } = state;
  const [wantsFlip, setWantsFlip] = useState(false);
  const cardSizes = useResponsiveCardSize();

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

  const handleCardClick = useCallback(
    (boardPlayerIdx: number, position: number) => {
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
    },
    [isMyTurn, isInitialFlips, isDeckDrawAction, isDiscardPlacement, wantsFlip, boards, playerIndex, onAction]
  );

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

  // Drawn card display
  const hasDrawnCard = (isDeckDrawAction && action_needed.drawn_card != null)
    || isDiscardPlacement;

  return (
    <div className="space-y-4">
      {/* Status */}
      <div className="text-center">
        <p className={cn(
          'text-sm font-medium',
          isMyTurn ? 'text-primary' : 'text-muted-foreground'
        )}>
          {prompt}
        </p>
        {state.is_final_turn && (
          <Badge variant="destructive" className="mt-1">Final Turn</Badge>
        )}
      </div>

      {/* Deck / Discard / Drawn Card row */}
      <div className="flex items-center justify-center gap-2 sm:gap-4">
        {/* Deck */}
        <div className="flex flex-col items-center gap-1">
          <span className="text-xs text-muted-foreground">Deck ({state.deck_remaining})</span>
          <button
            onClick={handleDrawDeck}
            disabled={!isMyTurn || !isChooseDraw}
            className={cn(
              'transition-transform',
              isMyTurn && isChooseDraw && 'hover:scale-105 cursor-pointer ring-2 ring-blue-400 rounded-lg'
            )}
          >
            <SkyjoCard slot={{ Hidden: 0 }} size={cardSizes.draw} />
          </button>
        </div>

        {/* Discard piles */}
        {state.discard_tops.map((top, i) => (
          <div key={i} className="flex flex-col items-center gap-1">
            <span className="text-xs text-muted-foreground">Discard</span>
            <button
              onClick={() => handleDrawDiscard(i)}
              disabled={!isMyTurn || !isChooseDraw || top == null}
              className={cn(
                'transition-transform',
                isMyTurn && isChooseDraw && top != null && 'hover:scale-105 cursor-pointer ring-2 ring-blue-400 rounded-lg'
              )}
            >
              {top != null ? (
                <SkyjoCard slot={{ Revealed: top }} size={cardSizes.draw} />
              ) : (
                <SkyjoCard slot="Cleared" size={cardSizes.draw} />
              )}
            </button>
          </div>
        ))}

        {/* Drawn card */}
        <div className="flex flex-col items-center gap-1">
          <span className="text-xs text-muted-foreground">Drawn</span>
          {hasDrawnCard && isDeckDrawAction && action_needed.drawn_card != null ? (
            <div className="ring-2 ring-green-400 rounded-lg">
              <SkyjoCard slot={{ Revealed: action_needed.drawn_card }} size={cardSizes.draw} />
            </div>
          ) : hasDrawnCard && isDiscardPlacement ? (
            <div className="ring-2 ring-green-400 rounded-lg">
              <SkyjoCard slot={{ Revealed: action_needed.drawn_card }} size={cardSizes.draw} />
            </div>
          ) : (
            <SkyjoCard slot="Cleared" size={cardSizes.draw} />
          )}
        </div>
      </div>

      {/* Flip toggle for deck draw */}
      {isMyTurn && isDeckDrawAction && (
        <div className="flex justify-center">
          <Button
            variant={wantsFlip ? 'default' : 'outline'}
            size="sm"
            onClick={() => setWantsFlip(!wantsFlip)}
          >
            {wantsFlip ? 'Flip mode (click hidden card)' : 'Switch to flip mode'}
          </Button>
        </div>
      )}

      {/* Player boards */}
      <div className="flex flex-wrap gap-2 sm:gap-4 justify-center">
        {boards.map((board, boardPlayerIdx) => (
          <div
            key={boardPlayerIdx}
            className={cn(
              'rounded-lg border p-2 sm:p-3',
              boardPlayerIdx === current_player && 'border-primary border-2',
              boardPlayerIdx === playerIndex && 'bg-primary/5',
              boardPlayerIdx === state.going_out_player && 'border-orange-400 border-2',
            )}
          >
            <h4 className="text-xs sm:text-sm font-medium mb-1 sm:mb-2 flex items-center gap-1">
              {getPlayerName(state, boardPlayerIdx)}
              {boardPlayerIdx === playerIndex && (
                <Badge variant="outline" className="text-[10px] py-0">you</Badge>
              )}
              {boardPlayerIdx === state.going_out_player && (
                <span className="text-xs text-orange-500">(went out)</span>
              )}
            </h4>
            <div
              className="grid gap-0.5 sm:gap-1"
              style={{ gridTemplateColumns: `repeat(${num_cols}, 1fr)` }}
            >
              {Array.from({ length: num_rows }, (_, r) =>
                Array.from({ length: num_cols }, (_, c) => {
                  const idx = c * num_rows + r;
                  const interactive = getCardInteractive(boardPlayerIdx, idx);
                  return (
                    <button
                      key={idx}
                      onClick={() => handleCardClick(boardPlayerIdx, idx)}
                      disabled={!interactive}
                      className={cn(
                        'transition-transform',
                        interactive && 'hover:scale-105 cursor-pointer ring-2 ring-yellow-400 rounded-lg'
                      )}
                    >
                      <SkyjoCard slot={toSlot(board[idx])} size={cardSizes.board} />
                    </button>
                  );
                })
              ).flat()}
            </div>
          </div>
        ))}
      </div>
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
