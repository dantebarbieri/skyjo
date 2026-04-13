import { useState, useEffect, useRef } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { useRealtime, type RealtimeSpeed } from '@/hooks/use-realtime';
import PlayerBoard from './player-board';
import PileDisplay from './pile-display';
import type { GameHistory } from '../types';

interface RealtimeSectionProps {
  history: GameHistory | null;
  strategyNames: string[];
  onNeedNextGame: () => void;
}

export default function RealtimeSection({ history, strategyNames, onNeedNextGame }: RealtimeSectionProps) {
  const rt = useRealtime();
  const lastHistoryRef = useRef<GameHistory | null>(null);
  // Default to showing deck top unless a "Human" strategy is present
  const [showDeckTop, setShowDeckTop] = useState(!strategyNames.some(s => s === 'Human'));

  useEffect(() => {
    rt.setOnNeedNextGame(onNeedNextGame);
  }, [onNeedNextGame, rt]);

  useEffect(() => {
    if (history && history !== lastHistoryRef.current) {
      lastHistoryRef.current = history;
      rt.loadGame(history);
    }
    return () => {
      // Allow re-mount (e.g. React StrictMode) to re-trigger loadGame
      lastHistoryRef.current = null;
    };
  }, [history, rt]);

  const speeds: RealtimeSpeed[] = ['slow', 'normal', 'fast'];

  if (!rt.step && !rt.interstitial) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Live Game</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">Run a simulation to watch live games</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <CardTitle className="text-base">Live Game</CardTitle>
            {rt.gameNumber > 0 && (
              <Badge variant="outline">#{rt.gameNumber}</Badge>
            )}
          </div>
          <div className="flex items-center gap-3">
            <label className="flex items-center gap-1 text-xs text-muted-foreground">
              <Checkbox checked={showDeckTop} onCheckedChange={(v) => setShowDeckTop(!!v)} />
              Deck top
            </label>
            <span className="text-xs text-muted-foreground">Speed:</span>
            {speeds.map((s) => (
              <Button
                key={s}
                size="sm"
                variant={rt.speed === s ? 'default' : 'outline'}
                className="h-6 text-xs px-2"
                onClick={() => rt.changeSpeed(s)}
              >
                {s.charAt(0).toUpperCase() + s.slice(1)}
              </Button>
            ))}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {rt.interstitial ? (
          <div className="text-center py-8 space-y-2">
            <p className="text-lg font-semibold">Game {rt.interstitial.gameNumber} complete</p>
            <p className="text-sm text-muted-foreground">
              {rt.interstitial.winners.length === 1
                ? `Winner: Player ${rt.interstitial.winners[0] + 1} (${rt.interstitial.strategyNames[rt.interstitial.winners[0]]})`
                : `Winners: ${rt.interstitial.winners.map(w => `Player ${w + 1}`).join(', ')}`}
            </p>
            <p className="text-sm text-muted-foreground">
              Final scores: {rt.interstitial.scores.join(', ')}
            </p>
          </div>
        ) : rt.step ? (
          <div className="space-y-3">
            <div>
              <h3 className="text-sm font-semibold">
                Round {rt.step.roundIndex + 1} — {rt.step.stepLabel}
              </h3>
              <p className="text-xs text-muted-foreground">{rt.step.state.description}</p>
            </div>

            <PileDisplay state={rt.step.state} showDeckTop={showDeckTop} />

            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
              {rt.step.state.boards.map((board, p) => (
                <PlayerBoard
                  key={p}
                  board={board}
                  numRows={rt.step!.state.numRows}
                  numCols={rt.step!.state.numCols}
                  playerIndex={p}
                  strategyName={rt.strategyNames[p] ?? strategyNames[p] ?? ''}
                  state={rt.step!.state}
                  isRoundEnd={rt.step!.state.roundScores !== null}
                  lowestRoundScore={
                    rt.step!.state.roundScores
                      ? Math.min(...rt.step!.state.roundScores)
                      : Infinity
                  }
                />
              ))}
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
