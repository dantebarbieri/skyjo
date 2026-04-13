import { useRef, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { useReplay } from '@/hooks/use-replay';
import ReplayControls from './replay-controls';
import PlayerBoard from './player-board';
import PileDisplay from './pile-display';
import type { GameHistory } from '../types';

interface ReplaySectionProps {
  history: GameHistory;
  onClose: () => void;
}

export default function ReplaySection({ history, onClose }: ReplaySectionProps) {
  const replay = useReplay(history);
  const sectionRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    sectionRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [history]);

  const { step } = replay;
  const { state } = step;

  const isRoundEnd = state.roundScores !== null;
  let lowestRoundScore = Infinity;
  if (isRoundEnd && state.roundScores) {
    lowestRoundScore = Math.min(...state.roundScores);
  }

  return (
    <Card ref={sectionRef}>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle>Game Replay</CardTitle>
          <Button variant="ghost" size="sm" onClick={onClose}>Close</Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <ReplayControls
          currentStep={replay.currentStep}
          totalSteps={replay.totalSteps}
          playing={replay.playing}
          speed={replay.speed}
          pauseBetweenRounds={replay.pauseBetweenRounds}
          roundStarts={replay.roundStarts}
          history={history}
          activeRound={step.roundIndex}
          onPrev={replay.prev}
          onNext={replay.next}
          onToggleAutoplay={replay.toggleAutoplay}
          onSetSpeed={replay.setSpeed}
          onSetPauseBetweenRounds={replay.setPauseBetweenRounds}
          onJumpToRound={replay.jumpToRound}
          onSkipToRoundStart={replay.skipToRoundStart}
          onSkipToRoundEnd={replay.skipToRoundEnd}
        />

        <Collapsible>
          <CollapsibleTrigger className="text-sm text-muted-foreground hover:text-foreground transition-colors">
            Legend...
          </CollapsibleTrigger>
          <CollapsibleContent className="mt-2">
            <div className="flex gap-4 flex-wrap text-xs text-muted-foreground">
              <span><span className="inline-block w-3 h-3 rounded border-2 border-blue-500 mr-1 align-middle" /> Active player</span>
              <span><span className="inline-block w-3 h-3 rounded border-2 border-green-500 bg-green-50 mr-1 align-middle" /> Went out — solo lowest</span>
              <span><span className="inline-block w-3 h-3 rounded border-2 border-red-500 bg-red-50 mr-1 align-middle" /> Went out — penalized</span>
              <span><span className="inline-block w-3 h-3 rounded border-2 border-blue-400 bg-blue-50 mr-1 align-middle" /> Went out — safe</span>
              <span><strong>Bold</strong> = lowest round score</span>
            </div>
          </CollapsibleContent>
        </Collapsible>

        {/* Step header */}
        <div>
          <h3 className="text-base font-semibold">
            Round {step.roundIndex + 1} — {step.stepLabel}
          </h3>
          <p className="text-sm text-muted-foreground">{state.description}</p>
        </div>

        {/* Piles */}
        <PileDisplay state={state} />

        {/* Player boards */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
          {state.boards.map((board, p) => (
            <PlayerBoard
              key={p}
              board={board}
              numRows={state.numRows}
              numCols={state.numCols}
              playerIndex={p}
              strategyName={history.strategy_names[p]}
              state={state}
              isRoundEnd={isRoundEnd}
              lowestRoundScore={lowestRoundScore}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
