import { Card, CardContent } from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { cn } from '@/lib/utils';
import type { RoundRecord } from '@/hooks/use-interactive-game';

export function RoundScorecard({
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
                      const rawScore = round.rawRoundScores[i];
                      const wasPenalized = score !== rawScore;

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
                            {wasPenalized
                              ? <>{rawScore}→{score}</>
                              : score
                            }
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
          <span className="text-destructive">Red (raw→penalized)</span> = penalty applied |{' '}
          <span className="italic">Small number</span> = cumulative score
        </div>
      </CardContent>
    </Card>
  );
}
