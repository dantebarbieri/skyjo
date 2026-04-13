import SkyjoCard from './skyjo-card';
import { cn } from '@/lib/utils';
import { computeKnownScore, computeTrueScore, type ReplayState } from '@/lib/replay-engine';
import type { Slot } from '../types';

interface PlayerBoardProps {
  board: Slot[];
  numRows: number;
  numCols: number;
  playerIndex: number;
  strategyName: string;
  state: ReplayState;
  isRoundEnd: boolean;
  lowestRoundScore: number;
}

export default function PlayerBoard({
  board,
  numRows,
  numCols,
  playerIndex,
  strategyName,
  state,
  isRoundEnd,
  lowestRoundScore,
}: PlayerBoardProps) {
  const p = playerIndex;
  const isActive = state.currentPlayer === p;
  const isGoingOut = isRoundEnd && state.goingOutPlayer === p;

  let borderClass = 'border-border';
  if (isActive) {
    borderClass = 'border-blue-500 border-2';
  } else if (isGoingOut && state.roundScores) {
    const roundScore = state.roundScores[p];
    const isSoloLowest =
      roundScore === lowestRoundScore &&
      state.roundScores.filter((s) => s === lowestRoundScore).length === 1;
    const wouldBePenalized = roundScore > 0 && !isSoloLowest;

    if (isSoloLowest) {
      borderClass = 'border-green-500 border-2 bg-green-50 dark:bg-green-950/20';
    } else if (wouldBePenalized) {
      borderClass = 'border-red-500 border-2 bg-red-50 dark:bg-red-950/20';
    } else {
      borderClass = 'border-blue-400 border-2 bg-blue-50 dark:bg-blue-950/20';
    }
  }

  const known = computeKnownScore(board);
  const true_ = computeTrueScore(board);

  let scoreText: string;
  if (known === true_) {
    scoreText = `Score: ${known}`;
    if (isRoundEnd && state.goingOutPlayer === p && state.roundScores && state.roundScores[p] !== known) {
      scoreText += ` (${state.roundScores[p]})`;
    }
  } else {
    scoreText = `Known: ${known} | True: ${true_}`;
  }

  const isLowestScore = isRoundEnd && state.roundScores && state.roundScores[p] === lowestRoundScore;

  return (
    <div className={cn('rounded-lg border p-3 transition-colors', borderClass)}>
      <h4 className="text-sm font-medium mb-2">
        Player {p + 1} <span className="text-muted-foreground">({strategyName})</span>
      </h4>

      <div
        className="grid gap-1"
        style={{ gridTemplateColumns: `repeat(${numCols}, 1fr)` }}
      >
        {/* Convert column-major to row-major for display */}
        {Array.from({ length: numRows }, (_, r) =>
          Array.from({ length: numCols }, (_, c) => {
            const idx = c * numRows + r;
            const slot = board[idx];
            return (
              <SkyjoCard
                key={idx}
                slot={slot}
                size="sm"
              />
            );
          })
        ).flat()}
      </div>

      <div className={cn(
        'text-xs mt-2 text-muted-foreground',
        isLowestScore && 'font-bold text-foreground',
      )}>
        {scoreText}
      </div>
    </div>
  );
}
