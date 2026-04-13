import { useState, useCallback, useRef } from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useDocumentTitle } from '@/hooks/use-document-title';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import SkyjoCard from '@/components/skyjo-card';
import { useWasmContext } from '@/contexts/wasm-context';
import { useInteractiveGame, type RoundRecord } from '@/hooks/use-interactive-game';
import { useBotTurns } from '@/hooks/use-bot-turns';
import { cn } from '@/lib/utils';
import { useResponsiveCardSize } from '@/hooks/use-responsive-card-size';
import type {
  PlayConfig,
  PlayerType,
  BotSpeed,
  InteractiveGameState,
  ActionNeeded,
  PlayerAction,
  VisibleSlot,
  Slot,
} from '@/types';
import { BOT_SPEED_LABELS } from '@/types';

// --- Helpers ---

/** Convert a VisibleSlot to a Slot for SkyjoCard rendering */
function toSlot(vs: VisibleSlot): Slot {
  if (vs === 'Hidden') return { Hidden: 0 };
  if (vs === 'Cleared') return 'Cleared';
  return { Revealed: vs.Revealed };
}

function getPlayerName(state: InteractiveGameState, index: number): string {
  return state.player_names[index] || `Player ${index + 1}`;
}

/** Compute the sum of all revealed card values on a board */
function computeVisibleScore(board: VisibleSlot[]): number {
  let sum = 0;
  for (const slot of board) {
    if (typeof slot === 'object' && slot !== null && 'Revealed' in slot) {
      sum += slot.Revealed;
    }
  }
  return sum;
}

// --- Game Setup ---

function GameSetup({
  rules,
  strategies,
  onStart,
  hasSavedGame,
  onResume,
  onImport,
}: {
  rules: string[];
  strategies: string[];
  onStart: (config: PlayConfig) => void;
  hasSavedGame: boolean;
  onResume: () => void;
  onImport: (json: string) => void;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [numPlayers, setNumPlayers] = useState(2);
  const [playerNames, setPlayerNames] = useState<string[]>(['', '']);
  const [playerTypes, setPlayerTypes] = useState<PlayerType[]>(['Human', 'Human']);
  const [selectedRules, setSelectedRules] = useState('Standard');
  const [seed, setSeed] = useState(() => Math.floor(Math.random() * 1000000));

  // Build dropdown options: "Human" + "Bot - <Strategy>" for each strategy
  const typeOptions: { value: PlayerType; label: string }[] = [
    { value: 'Human', label: 'Human' },
    ...strategies.map((s) => ({
      value: `Bot:${s}` as PlayerType,
      label: `Bot - ${s}`,
    })),
  ];

  const handleNumPlayersChange = useCallback((value: string) => {
    const n = parseInt(value);
    setNumPlayers(n);
    setPlayerNames((prev) => {
      const next = [...prev];
      while (next.length < n) next.push('');
      return next.slice(0, n);
    });
    setPlayerTypes((prev) => {
      const next = [...prev];
      while (next.length < n) next.push('Human');
      return next.slice(0, n);
    });
  }, []);

  const handleNameChange = useCallback((index: number, name: string) => {
    setPlayerNames((prev) => {
      const next = [...prev];
      next[index] = name;
      return next;
    });
  }, []);

  const handleTypeChange = useCallback((index: number, type: PlayerType) => {
    setPlayerTypes((prev) => {
      const next = [...prev];
      next[index] = type;
      return next;
    });
  }, []);

  const handleStart = useCallback(() => {
    const names = playerNames.map((name, i) => {
      if (name.trim()) return name.trim();
      if (playerTypes[i] === 'Human') return `Player ${i + 1}`;
      return `Bot (${playerTypes[i].slice(4)})`;
    });
    onStart({
      num_players: numPlayers,
      player_names: names,
      player_types: playerTypes.slice(0, numPlayers),
      rules: selectedRules,
      seed,
    });
  }, [numPlayers, playerNames, playerTypes, selectedRules, seed, onStart]);

  return (
    <Card>
      <CardContent className="pt-6 space-y-4">
        <h2 className="text-lg font-semibold">Game Setup</h2>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Number of Players</label>
            <Select value={String(numPlayers)} onValueChange={handleNumPlayersChange}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[2, 3, 4, 5, 6, 7, 8].map((n) => (
                  <SelectItem key={n} value={String(n)}>
                    {n} Players
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <label className="text-sm font-medium">Rules</label>
            <Select value={selectedRules} onValueChange={setSelectedRules}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {rules.map((r) => (
                  <SelectItem key={r} value={r}>
                    {r}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Players</label>
          <div className="space-y-2">
            {Array.from({ length: numPlayers }, (_, i) => (
              <div key={i} className="flex gap-2 items-center">
                <span className="text-sm text-muted-foreground w-6 shrink-0">{i + 1}.</span>
                <Select
                  value={playerTypes[i] || 'Human'}
                  onValueChange={(v) => handleTypeChange(i, v as PlayerType)}
                >
                  <SelectTrigger className="w-28 sm:w-40 shrink-0">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {typeOptions.map((opt) => (
                      <SelectItem key={opt.value} value={opt.value}>
                        {opt.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Input
                  placeholder={
                    playerTypes[i] === 'Human'
                      ? `Player ${i + 1}`
                      : `Bot (${playerTypes[i]?.slice(4) || ''})`
                  }
                  value={playerNames[i] || ''}
                  onChange={(e) => handleNameChange(i, e.target.value)}
                />
              </div>
            ))}
          </div>
        </div>

        <div className="space-y-1.5">
          <label className="text-sm font-medium">Seed</label>
          <div className="flex gap-2">
            <Input
              type="number"
              value={seed}
              onChange={(e) => setSeed(parseInt(e.target.value) || 0)}
              className="w-28 sm:w-40"
            />
            <Button
              variant="outline"
              size="sm"
              onClick={() => setSeed(Math.floor(Math.random() * 1000000))}
            >
              Random
            </Button>
          </div>
        </div>

        <Button onClick={handleStart} className="w-full">
          Start Game
        </Button>

        {hasSavedGame && (
          <Button onClick={onResume} variant="secondary" className="w-full">
            Resume Saved Game
          </Button>
        )}

        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            className="flex-1"
            onClick={() => fileInputRef.current?.click()}
          >
            Import Game
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (!file) return;
              const reader = new FileReader();
              reader.onload = () => {
                if (typeof reader.result === 'string') {
                  onImport(reader.result);
                }
              };
              reader.readAsText(file);
              e.target.value = '';
            }}
          />
        </div>
      </CardContent>
    </Card>
  );
}

// --- Interactive Board ---

function PlayBoard({
  state,
  onAction,
}: {
  state: InteractiveGameState;
  onAction: (action: PlayerAction) => void;
}) {
  const { action_needed, boards, num_rows, num_cols, current_player } = state;
  const [wantsFlip, setWantsFlip] = useState(false);
  const cardSizes = useResponsiveCardSize();

  // Get the player whose turn it is for initial flips
  const activePlayer = action_needed.type === 'ChooseInitialFlips'
    ? action_needed.player
    : current_player;

  const handleCardClick = useCallback(
    (playerIndex: number, position: number) => {
      if (action_needed.type === 'ChooseInitialFlips') {
        // Only the flip player can click during initial flips
        if (playerIndex !== action_needed.player) return;
        const slot = boards[playerIndex][position];
        if (slot === 'Hidden') {
          onAction({ type: 'InitialFlip', position });
        }
        return;
      }

      if (playerIndex !== current_player) return;

      switch (action_needed.type) {
        case 'ChooseDeckDrawAction': {
          if (wantsFlip) {
            const slot = boards[current_player][position];
            if (slot === 'Hidden') {
              onAction({ type: 'DiscardAndFlip', position });
              setWantsFlip(false);
            }
          } else {
            const slot = boards[current_player][position];
            if (slot !== 'Cleared') {
              onAction({ type: 'KeepDeckDraw', position });
            }
          }
          break;
        }
        case 'ChooseDiscardDrawPlacement': {
          const slot = boards[current_player][position];
          if (slot !== 'Cleared') {
            onAction({ type: 'PlaceDiscardDraw', position });
          }
          break;
        }
      }
    },
    [action_needed, current_player, boards, wantsFlip, onAction]
  );

  const handleDrawDeck = useCallback(() => {
    if (action_needed.type === 'ChooseDraw') {
      onAction({ type: 'DrawFromDeck' });
    }
  }, [action_needed, onAction]);

  const handleDrawDiscard = useCallback(
    (pileIndex: number) => {
      if (action_needed.type === 'ChooseDraw') {
        onAction({ type: 'DrawFromDiscard', pile_index: pileIndex });
      }
    },
    [action_needed, onAction]
  );

  const handleToggleFlipMode = useCallback(() => {
    setWantsFlip((prev) => !prev);
  }, []);

  // Determine what's interactive
  const isChooseDraw = action_needed.type === 'ChooseDraw';
  const isDeckDrawAction = action_needed.type === 'ChooseDeckDrawAction';
  const isDiscardPlacement = action_needed.type === 'ChooseDiscardDrawPlacement';
  const isInitialFlips = action_needed.type === 'ChooseInitialFlips';

  // Prompt message
  let prompt = '';
  if (isInitialFlips) {
    const remaining = action_needed.count;
    prompt = `Click ${remaining} hidden card${remaining !== 1 ? 's' : ''} to flip`;
  } else if (isChooseDraw) {
    prompt = 'Draw from the deck or discard pile';
  } else if (isDeckDrawAction) {
    const card = action_needed.drawn_card;
    prompt = wantsFlip
      ? `Click a hidden card to flip (discarding the ${card})`
      : `Click a card to replace with your ${card}, or discard & flip instead`;
  } else if (isDiscardPlacement) {
    const card = action_needed.drawn_card;
    prompt = `Click a card to replace with your ${card}`;
  }

  // Which cards are clickable on a given player's board
  const getCardInteractive = (playerIdx: number, pos: number): boolean => {
    if (isInitialFlips) {
      if (playerIdx !== action_needed.player) return false;
      return boards[playerIdx][pos] === 'Hidden';
    }
    if (playerIdx !== current_player) return false;
    const slot = boards[playerIdx][pos];
    if (isDeckDrawAction && wantsFlip) return slot === 'Hidden';
    if (isDeckDrawAction && !wantsFlip) return slot !== 'Cleared';
    if (isDiscardPlacement) return slot !== 'Cleared';
    return false;
  };

  return (
    <div className="space-y-4">
      {/* Status bar */}
      <div className="text-center space-y-1">
        <h3 className="text-lg font-semibold">
          {getPlayerName(state, activePlayer)}'s Turn
          {state.is_final_turn && (
            <span className="text-sm text-orange-500 ml-2">(Final Turn!)</span>
          )}
        </h3>
        <p className="text-sm text-muted-foreground">
          Round {state.round_number + 1}
        </p>
        <p className="text-sm font-medium text-primary">{prompt}</p>
      </div>

      {/* Draw area */}
      {(isChooseDraw || isDeckDrawAction || isDiscardPlacement) && (
        <div className="flex items-center justify-center gap-3 sm:gap-6 md:gap-8">
          {/* Deck */}
          <button
            onClick={handleDrawDeck}
            disabled={!isChooseDraw}
            className={cn(
              'flex flex-col items-center gap-1 transition-transform',
              isChooseDraw && 'hover:scale-105 cursor-pointer'
            )}
          >
            <span className="text-xs text-muted-foreground">
              Deck ({state.deck_remaining})
            </span>
            <div
              className={cn(
                'rounded-lg',
                isChooseDraw && 'ring-2 ring-blue-400'
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
                !isChooseDraw ||
                top === null ||
                !action_needed.drawable_piles?.includes(pileIdx)
              }
              className={cn(
                'flex flex-col items-center gap-1 transition-transform',
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

          {/* Drawn card display */}
          {(isDeckDrawAction || isDiscardPlacement) && (
            <div className="flex flex-col items-center gap-1">
              <span className="text-xs text-muted-foreground">Drawn</span>
              <div className="ring-2 ring-green-400 rounded-lg">
                <SkyjoCard
                  slot={{
                    Revealed:
                      action_needed.type === 'ChooseDeckDrawAction'
                        ? action_needed.drawn_card
                        : action_needed.type === 'ChooseDiscardDrawPlacement'
                          ? action_needed.drawn_card
                          : 0,
                  }}
                  size={cardSizes.draw}
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Discard & Flip toggle */}
      {isDeckDrawAction && (
        <div className="flex justify-center">
          <Button
            variant={wantsFlip ? 'default' : 'outline'}
            size="sm"
            onClick={handleToggleFlipMode}
          >
            {wantsFlip ? 'Back to Place Mode' : 'Discard & Flip Instead'}
          </Button>
        </div>
      )}

      {/* Undo discard draw */}
      {isDiscardPlacement && (
        <div className="flex justify-center">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onAction({ type: 'UndoDrawFromDiscard' })}
          >
            Undo — Put Back & Choose Again
          </Button>
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
        {boards.map((board, playerIdx) => {
          const isActive = playerIdx === activePlayer;
          const cardSize = isActive ? cardSizes.boardActive : cardSizes.board;

          return (
            <div
              key={playerIdx}
              className={cn(
                'rounded-lg border p-3 transition-colors',
                isActive
                  ? 'border-blue-500 border-2'
                  : 'border-border'
              )}
            >
              <h4 className="text-sm font-medium mb-2">
                {getPlayerName(state, playerIdx)}
                {playerIdx === state.going_out_player && (
                  <span className="text-xs text-orange-500 ml-1">(went out)</span>
                )}
              </h4>
              <div
                className="grid gap-0.5 sm:gap-1"
                style={{ gridTemplateColumns: `repeat(${num_cols}, 1fr)` }}
              >
                {Array.from({ length: num_rows }, (_, r) =>
                  Array.from({ length: num_cols }, (_, c) => {
                    const idx = c * num_rows + r;
                    const slot = board[idx];
                    const interactive = getCardInteractive(playerIdx, idx);

                    return (
                      <button
                        key={idx}
                        onClick={() => interactive && handleCardClick(playerIdx, idx)}
                        disabled={!interactive}
                        className={cn(
                          'transition-transform',
                          interactive && 'hover:scale-110 cursor-pointer'
                        )}
                      >
                        <SkyjoCard
                          slot={toSlot(slot)}
                          size={cardSize}
                          highlight={interactive}
                        />
                      </button>
                    );
                  })
                ).flat()}
              </div>
              <div className="text-xs mt-1 space-y-0.5">
                <div className="text-muted-foreground">
                  Visible: {computeVisibleScore(board)}
                </div>
                {state.cumulative_scores[playerIdx] !== 0 && (
                  <div className="text-muted-foreground">
                    Cumulative: {state.cumulative_scores[playerIdx]}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// --- Round Summary ---

function RoundSummary({
  state,
  actionNeeded,
  onContinue,
}: {
  state: InteractiveGameState;
  actionNeeded: ActionNeeded;
  onContinue: () => void;
}) {
  if (actionNeeded.type !== 'RoundOver') return null;

  const { round_scores, cumulative_scores, going_out_player, end_of_round_clears } = actionNeeded;

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

        {/* All boards revealed */}
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
                      <SkyjoCard
                        key={idx}
                        slot={toSlot(board[idx])}
                        size="sm"
                      />
                    );
                  })
                ).flat()}
              </div>
            </div>
          ))}
        </div>

        {/* Score table */}
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
              {state.player_names.map((name, i) => (
                <tr key={i} className="border-b last:border-0">
                  <td className="py-2 pr-4 font-medium">
                    {name}
                    {i === going_out_player && ' *'}
                  </td>
                  <td className="text-center py-2 px-2">{round_scores[i]}</td>
                  <td className="text-center py-2 px-2 font-bold">
                    {cumulative_scores[i]}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <Button onClick={onContinue} className="w-full">
          Next Round
        </Button>
      </CardContent>
    </Card>
  );
}

// --- Game Over ---

function GameOver({
  state,
  actionNeeded,
  onPlayAgain,
}: {
  state: InteractiveGameState;
  actionNeeded: ActionNeeded;
  onPlayAgain: () => void;
}) {
  if (actionNeeded.type !== 'GameOver') return null;

  const { final_scores, winners } = actionNeeded;
  const winnerNames = winners.map((i) => getPlayerName(state, i)).join(' & ');

  return (
    <Card>
      <CardContent className="pt-6 space-y-6">
        <div className="text-center space-y-2">
          <h2 className="text-2xl font-bold">Game Over!</h2>
          <p className="text-xl text-primary font-semibold">
            {winners.length > 1 ? 'Winners' : 'Winner'}: {winnerNames}
          </p>
        </div>

        {/* Final standings */}
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b">
                <th className="text-left py-2 pr-4">Rank</th>
                <th className="text-left py-2 pr-4">Player</th>
                <th className="text-center py-2 px-2">Final Score</th>
              </tr>
            </thead>
            <tbody>
              {final_scores
                .map((score, i) => ({ score, name: getPlayerName(state, i), index: i }))
                .sort((a, b) => a.score - b.score)
                .map((entry, rank) => (
                  <tr
                    key={entry.index}
                    className={cn(
                      'border-b last:border-0',
                      winners.includes(entry.index) && 'bg-green-50 dark:bg-green-950/20'
                    )}
                  >
                    <td className="py-2 pr-4">{rank + 1}</td>
                    <td className="py-2 pr-4 font-medium">
                      {entry.name}
                      {winners.includes(entry.index) && ' *'}
                    </td>
                    <td className="text-center py-2 px-2 font-bold">{entry.score}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>

        <Button onClick={onPlayAgain} className="w-full" variant="default">
          Play Again
        </Button>
      </CardContent>
    </Card>
  );
}

// --- Round Scorecard ---

function RoundScorecard({
  roundHistory,
  playerNames,
  currentCumulativeScores,
}: {
  roundHistory: RoundRecord[];
  playerNames: string[];
  currentCumulativeScores: number[];
}) {
  if (roundHistory.length === 0) return null;

  return (
    <Card className="mt-4">
      <CardContent className="pt-4">
        <h3 className="text-sm font-semibold mb-2">Score Sheet</h3>
        <div className="rounded-lg border overflow-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-14 sm:w-20 text-center">Round</TableHead>
                {playerNames.map((name, i) => (
                  <TableHead key={i} className="text-center min-w-14 sm:min-w-20">
                    {name}
                  </TableHead>
                ))}
              </TableRow>
            </TableHeader>
            <TableBody>
              {roundHistory.map((round) => {
                const lowestRoundScore = Math.min(...round.roundScores);
                return (
                  <TableRow key={round.roundNumber}>
                    <TableCell className="text-center font-medium text-sm">
                      {round.roundNumber + 1}
                    </TableCell>
                    {round.roundScores.map((score, i) => {
                      const isGoingOut = round.goingOutPlayer === i;
                      const isLowest = score === lowestRoundScore;
                      const wasPenalized = isGoingOut && score > 0 &&
                        !round.roundScores.every((s, j) => j === i || s > score);

                      return (
                        <TableCell
                          key={i}
                          className={cn(
                            'text-center',
                            isGoingOut && 'bg-muted/50',
                          )}
                        >
                          <div className={cn(
                            'text-sm',
                            isLowest && 'font-bold',
                            wasPenalized && 'text-destructive',
                          )}>
                            {score}
                            {isGoingOut && <span className="text-[10px] ml-0.5">*</span>}
                          </div>
                          <div className="text-[10px] text-muted-foreground">
                            {round.cumulativeScores[i]}
                          </div>
                        </TableCell>
                      );
                    })}
                  </TableRow>
                );
              })}
              {/* Current totals row */}
              <TableRow className="bg-muted/30 font-semibold">
                <TableCell className="text-center text-sm">Total</TableCell>
                {currentCumulativeScores.map((score, i) => (
                  <TableCell key={i} className="text-center text-sm">
                    {score}
                  </TableCell>
                ))}
              </TableRow>
            </TableBody>
          </Table>
        </div>
        <div className="text-[10px] text-muted-foreground mt-1">
          * = went out | <strong>Bold</strong> = lowest round score |{' '}
          <span className="text-destructive">Red</span> = penalized |{' '}
          <span className="italic">Small number</span> = cumulative score
        </div>
      </CardContent>
    </Card>
  );
}

// --- Main Play Route ---

const BOT_SPEED_STORAGE_KEY = 'skyjo-bot-speed';

function loadBotSpeed(): BotSpeed {
  try {
    const v = localStorage.getItem(BOT_SPEED_STORAGE_KEY);
    if (v === 'slow' || v === 'normal' || v === 'fast' || v === 'instant') return v;
  } catch { /* ignore */ }
  return 'normal';
}

export default function PlayRoute() {
  useDocumentTitle('Play');
  const wasm = useWasmContext();
  const game = useInteractiveGame();
  const [botSpeed, setBotSpeed] = useState<BotSpeed>(loadBotSpeed);

  const hasBots = game.playerTypes.some((t) => t !== 'Human');

  const handleBotSpeedChange = useCallback((value: string) => {
    const speed = value as BotSpeed;
    setBotSpeed(speed);
    try { localStorage.setItem(BOT_SPEED_STORAGE_KEY, speed); } catch { /* ignore */ }
  }, []);

  // Auto-play bot turns
  useBotTurns({
    gameState: game.gameState,
    phase: game.phase,
    playerTypes: game.playerTypes,
    botSpeed,
    applyBotTurn: game.applyBotTurn,
    continueToNextRound: game.continueToNextRound,
    showStartingPlayer: game.showStartingPlayer,
  });

  return (
    <>
      <h1 className="text-2xl sm:text-3xl font-bold mb-6">Play Skyjo</h1>

      {game.error && (
        <div className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive mb-4">
          {game.error}
        </div>
      )}

      {game.phase === 'setup' && (
        <GameSetup
          rules={wasm.rules}
          strategies={wasm.strategies}
          onStart={game.createGame}
          hasSavedGame={game.hasSavedGame}
          onResume={game.resumeGame}
          onImport={game.importGame}
        />
      )}

      {(game.phase === 'initial_flips' || game.phase === 'playing') &&
        game.gameState && (
          <Card>
            <CardContent className="pt-6">
              {hasBots && (
                <div className="flex items-center gap-2 mb-4">
                  <label className="text-sm font-medium">Bot Speed:</label>
                  <Select value={botSpeed} onValueChange={handleBotSpeedChange}>
                    <SelectTrigger className="w-24 sm:w-32">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {(Object.entries(BOT_SPEED_LABELS) as [BotSpeed, string][]).map(([key, label]) => (
                        <SelectItem key={key} value={key}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}
              <PlayBoard state={game.gameState} onAction={game.applyAction} />
            </CardContent>
          </Card>
        )}

      {game.phase === 'round_over' && game.gameState && game.actionNeeded && (
        <RoundSummary
          state={game.gameState}
          actionNeeded={game.actionNeeded}
          onContinue={game.continueToNextRound}
        />
      )}

      {game.phase === 'game_over' && game.gameState && game.actionNeeded && (
        <GameOver
          state={game.gameState}
          actionNeeded={game.actionNeeded}
          onPlayAgain={game.resetGame}
        />
      )}

      {/* Starting player popup */}
      <Dialog open={game.showStartingPlayer} onOpenChange={(open) => { if (!open) game.dismissStartingPlayer(); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Starting Player</DialogTitle>
            <DialogDescription asChild>
              <div className="space-y-3 pt-2">
                {game.gameState && (
                  <p className="text-base">
                    <span className="text-lg font-bold text-primary">
                      {getPlayerName(game.gameState, game.startingPlayerIndex)}
                    </span>
                    {' '}goes first
                    {game.gameState.round_number === 0
                      ? ' (highest initial flip sum)'
                      : ' (went out last round)'}
                  </p>
                )}
                <Button onClick={game.dismissStartingPlayer} className="w-full">
                  Start Playing
                </Button>
              </div>
            </DialogDescription>
          </DialogHeader>
        </DialogContent>
      </Dialog>

      {/* Round scorecard */}
      {game.phase !== 'setup' && game.gameState && game.roundHistory.length > 0 && (
        <RoundScorecard
          roundHistory={game.roundHistory}
          playerNames={game.gameState.player_names}
          currentCumulativeScores={game.gameState.cumulative_scores}
        />
      )}

      {game.phase !== 'setup' && (
        <div className="mt-4 flex justify-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              const json = game.exportGame();
              if (!json) return;
              const blob = new Blob([json], { type: 'application/json' });
              const url = URL.createObjectURL(blob);
              const a = document.createElement('a');
              a.href = url;
              a.download = `skyjo-save-${Date.now()}.json`;
              a.click();
              URL.revokeObjectURL(url);
            }}
          >
            Export Game
          </Button>
          <Button variant="outline" size="sm" onClick={game.resetGame}>
            Quit Game
          </Button>
        </div>
      )}
    </>
  );
}
