import { useParams, useNavigate } from 'react-router-dom';
import { useGameDetail } from '@/hooks/use-game-detail';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import ReplaySection from '@/components/replay-section';
import { ChevronLeft } from 'lucide-react';

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export default function GameDetailRoute() {
  const { gameId } = useParams<{ gameId: string }>();
  const navigate = useNavigate();
  const { game, replay, loadReplay, loading, replayLoading, error } =
    useGameDetail(gameId!);

  if (loading) {
    return (
      <div className="space-y-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigate('/leaderboard')}
        >
          <ChevronLeft className="h-4 w-4 mr-1" />
          Back to Leaderboard
        </Button>
        <div className="flex items-center justify-center py-12 text-muted-foreground">
          Loading game...
        </div>
      </div>
    );
  }

  if (error || !game) {
    return (
      <div className="space-y-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigate('/leaderboard')}
        >
          <ChevronLeft className="h-4 w-4 mr-1" />
          Back to Leaderboard
        </Button>
        <div className="flex items-center justify-center py-12 text-destructive">
          {error ?? 'Game not found'}
        </div>
      </div>
    );
  }

  const { players, rounds } = game;

  return (
    <div className="space-y-6">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => navigate('/leaderboard')}
      >
        <ChevronLeft className="h-4 w-4 mr-1" />
        Back to Leaderboard
      </Button>

      {/* Game metadata */}
      <div>
        <h1 className="text-2xl font-bold">Game Detail</h1>
        <div className="flex flex-wrap gap-4 mt-1 text-sm text-muted-foreground">
          <span>Room: {game.room_code}</span>
          <span>Rules: {game.rules}</span>
          <span>{game.num_players} players</span>
          <span>{game.num_rounds} rounds</span>
          <span>{formatDate(game.created_at)}</span>
        </div>
      </div>

      {/* Scorecard */}
      <Card>
        <CardHeader>
          <CardTitle>Score Sheet</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="rounded-lg border overflow-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-14 sm:w-20 text-center">
                    Round
                  </TableHead>
                  {players.map((p, i) => (
                    <TableHead key={i} className="text-center min-w-16 sm:min-w-24">
                      <div className="font-medium">{p.name}</div>
                      {p.is_bot && (
                        <div className="text-[10px] text-muted-foreground font-normal">
                          bot
                        </div>
                      )}
                    </TableHead>
                  ))}
                </TableRow>
              </TableHeader>
              <TableBody>
                {rounds.map((round) => {
                  const lowestAdjusted = Math.min(
                    ...round.scores.map((s) => s.adjusted_score),
                  );
                  return (
                    <TableRow key={round.round_number}>
                      <TableCell className="text-center font-medium text-sm">
                        {round.round_number}
                      </TableCell>
                      {round.scores.map((score) => (
                        <TableCell
                          key={score.player_index}
                          className={cn(
                            'text-center',
                            score.went_out && 'bg-muted/50',
                          )}
                        >
                          <div
                            className={cn(
                              'text-sm',
                              score.adjusted_score === lowestAdjusted &&
                                'font-bold',
                              score.was_penalized && 'text-destructive',
                            )}
                          >
                            {score.adjusted_score}
                            {score.went_out && (
                              <span className="text-[10px] ml-0.5">*</span>
                            )}
                          </div>
                          <div className="text-[10px] text-muted-foreground">
                            {score.cumulative_score}
                          </div>
                        </TableCell>
                      ))}
                    </TableRow>
                  );
                })}
                {/* Final totals */}
                <TableRow className="bg-muted/30 font-semibold">
                  <TableCell className="text-center text-sm">Total</TableCell>
                  {players.map((p, i) => (
                    <TableCell
                      key={i}
                      className={cn(
                        'text-center text-sm',
                        p.is_winner &&
                          'text-green-600 dark:text-green-400',
                      )}
                    >
                      {p.final_score}
                      {p.is_winner && (
                        <span className="text-[10px] ml-0.5">W</span>
                      )}
                    </TableCell>
                  ))}
                </TableRow>
              </TableBody>
            </Table>
          </div>

          <div className="mt-2 text-[10px] text-muted-foreground">
            * = went out | <strong>Bold</strong> = lowest round score |{' '}
            <span className="text-destructive">Red</span> = penalized |{' '}
            <span className="text-green-600">W</span> = winner |{' '}
            <span className="italic">Small number</span> = cumulative score
          </div>
        </CardContent>
      </Card>

      {/* Replay section */}
      {!replay && (
        <Card>
          <CardContent className="flex items-center justify-center py-8">
            <Button onClick={loadReplay} disabled={replayLoading}>
              {replayLoading ? 'Loading Replay...' : 'Load Replay'}
            </Button>
          </CardContent>
        </Card>
      )}
      {replay && (
        <ReplaySection
          history={replay}
          onClose={() => navigate('/leaderboard')}
        />
      )}
    </div>
  );
}
