import { Button } from '@/components/ui/button';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { SkipBack, SkipForward, ChevronLeft, ChevronRight } from 'lucide-react';
import type { GameHistory } from '../types';

interface ReplayControlsProps {
  currentStep: number;
  totalSteps: number;
  playing: boolean;
  speed: number;
  pauseBetweenRounds: boolean;
  roundStarts: number[];
  history: GameHistory;
  activeRound: number;
  onPrev: () => void;
  onNext: () => void;
  onToggleAutoplay: () => void;
  onSetSpeed: (speed: number) => void;
  onSetPauseBetweenRounds: (value: boolean) => void;
  onJumpToRound: (roundIdx: number) => void;
  onSkipToRoundStart: () => void;
  onSkipToRoundEnd: () => void;
}

export default function ReplayControls({
  currentStep,
  totalSteps,
  playing,
  speed,
  pauseBetweenRounds,
  history,
  activeRound,
  onPrev,
  onNext,
  onToggleAutoplay,
  onSetSpeed,
  onSetPauseBetweenRounds,
  onJumpToRound,
  onSkipToRoundStart,
  onSkipToRoundEnd,
}: ReplayControlsProps) {
  return (
    <div className="space-y-3">
      {/* Round navigation */}
      <div className="flex items-center gap-1.5 sm:gap-2 flex-wrap">
        <span className="text-sm font-medium">Rounds:</span>
        {history.rounds.map((r, i) => (
          <Button
            key={i}
            size="sm"
            variant={i === activeRound ? 'default' : 'outline'}
            className="h-8 sm:h-7 text-xs"
            onClick={() => onJumpToRound(i)}
          >
            R{i + 1} ({r.turns.length})
            {r.truncated && (
              <Badge variant="destructive" className="ml-1 text-[8px] px-0.5 py-0">!</Badge>
            )}
          </Button>
        ))}
      </div>

      {/* Step controls */}
      <div className="flex items-center gap-1.5 sm:gap-2 flex-wrap">
        <Button size="sm" variant="outline" onClick={onSkipToRoundStart}>
          <SkipBack className="h-4 w-4 sm:hidden" />
          <span className="hidden sm:inline">Round Start</span>
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={onPrev}
          disabled={currentStep === 0}
        >
          <ChevronLeft className="h-4 w-4 sm:hidden" />
          <span className="hidden sm:inline">Previous</span>
        </Button>
        <span className="text-sm text-muted-foreground font-mono min-w-20 sm:min-w-32 text-center">
          {currentStep + 1} / {totalSteps}
        </span>
        <Button
          size="sm"
          variant="outline"
          onClick={onNext}
          disabled={currentStep === totalSteps - 1}
        >
          <ChevronRight className="h-4 w-4 sm:hidden" />
          <span className="hidden sm:inline">Next</span>
        </Button>
        <Button size="sm" variant="outline" onClick={onSkipToRoundEnd}>
          <SkipForward className="h-4 w-4 sm:hidden" />
          <span className="hidden sm:inline">Round End</span>
        </Button>
      </div>

      {/* Autoplay controls */}
      <div className="flex items-center gap-3 flex-wrap">
        <Button
          size="sm"
          onClick={onToggleAutoplay}
          variant={playing ? 'destructive' : 'default'}
        >
          {playing ? 'Pause' : 'Play'}
        </Button>
        <div className="flex items-center gap-1.5">
          <span className="text-sm">Speed:</span>
          <Select value={String(speed)} onValueChange={(v) => onSetSpeed(parseInt(v))}>
            <SelectTrigger className="w-20 sm:w-24 h-8">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="1500">Slow</SelectItem>
              <SelectItem value="600">Normal</SelectItem>
              <SelectItem value="150">Fast</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex items-center gap-1.5">
          <Checkbox
            id="pause-rounds"
            checked={pauseBetweenRounds}
            onCheckedChange={(checked) => onSetPauseBetweenRounds(checked === true)}
          />
          <label htmlFor="pause-rounds" className="text-sm cursor-pointer">
            Pause between rounds
          </label>
        </div>
      </div>
    </div>
  );
}
