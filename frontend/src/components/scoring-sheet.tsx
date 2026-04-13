import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { cn } from '@/lib/utils';
import type { GameHistory } from '../types';

interface ScoringSheetProps {
  history: GameHistory;
  onClose: () => void;
}

export default function ScoringSheet({ history, onClose }: ScoringSheetProps) {
  const { rounds, num_players, strategy_names, winners, final_scores } = history;

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold text-sm">
          Score Sheet — Game #{history.seed}
        </h3>
        <Button variant="ghost" size="sm" onClick={onClose} className="h-6 text-xs">
          Close
        </Button>
      </div>

      <div className="rounded-lg border overflow-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-20 text-center">Round</TableHead>
              {Array.from({ length: num_players }, (_, p) => (
                <TableHead key={p} className="text-center min-w-24">
                  <div className="font-medium">P{p + 1}</div>
                  <div className="text-[10px] text-muted-foreground font-normal">
                    {strategy_names[p]}
                  </div>
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rounds.map((round, ri) => {
              const lowestRoundScore = Math.min(...round.round_scores);
              const prevCumulative = ri === 0
                ? new Array(num_players).fill(0)
                : rounds[ri - 1].cumulative_scores;

              return (
                <TableRow key={ri}>
                  <TableCell className="text-center font-medium text-sm">
                    {ri + 1}
                    {round.truncated && (
                      <span className="text-destructive text-[10px] ml-1">!</span>
                    )}
                  </TableCell>
                  {Array.from({ length: num_players }, (_, p) => {
                    const roundScore = round.round_scores[p];
                    const cumScore = round.cumulative_scores[p];
                    const isGoingOut = round.going_out_player === p;
                    const isLowest = roundScore === lowestRoundScore;
                    // Check if penalty was applied
                    const expectedCum = prevCumulative[p] + roundScore;
                    const wasPenalized = cumScore !== expectedCum;

                    return (
                      <TableCell
                        key={p}
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
                          {roundScore}
                          {isGoingOut && <span className="text-[10px] ml-0.5">*</span>}
                        </div>
                        <div className="text-[10px] text-muted-foreground">
                          {cumScore}
                        </div>
                      </TableCell>
                    );
                  })}
                </TableRow>
              );
            })}
            {/* Final totals row */}
            <TableRow className="bg-muted/30 font-semibold">
              <TableCell className="text-center text-sm">Total</TableCell>
              {Array.from({ length: num_players }, (_, p) => (
                <TableCell
                  key={p}
                  className={cn(
                    'text-center text-sm',
                    winners.includes(p) && 'text-green-600 dark:text-green-400',
                  )}
                >
                  {final_scores[p]}
                  {winners.includes(p) && <span className="text-[10px] ml-0.5">W</span>}
                </TableCell>
              ))}
            </TableRow>
          </TableBody>
        </Table>
      </div>

      <div className="text-[10px] text-muted-foreground">
        * = went out | <strong>Bold</strong> = lowest round score |{' '}
        <span className="text-destructive">Red</span> = penalized |{' '}
        <span className="text-green-600">W</span> = winner |{' '}
        <span className="italic">Small number</span> = cumulative score
      </div>
    </div>
  );
}
