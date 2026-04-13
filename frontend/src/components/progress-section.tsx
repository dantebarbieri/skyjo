import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import type { SimStatus } from '@/hooks/use-simulation';
import type { ProgressStats } from '../types';

interface ProgressSectionProps {
  status: SimStatus;
  gamesCompleted: number;
  totalGames: number;
  elapsedMs: number;
  stats: ProgressStats | null;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(1)}s`;
  const min = Math.floor(sec / 60);
  const remSec = (sec % 60).toFixed(0);
  return `${min}m ${remSec}s`;
}

const STATUS_VARIANT: Record<SimStatus, 'default' | 'secondary' | 'destructive' | 'outline'> = {
  idle: 'secondary',
  running: 'default',
  paused: 'outline',
  complete: 'secondary',
  cached: 'secondary',
};

const STATUS_LABEL: Record<SimStatus, string> = {
  idle: 'Idle',
  running: 'Running',
  paused: 'Paused',
  complete: 'Complete',
  cached: 'Cached',
};

export default function ProgressSection({
  status,
  gamesCompleted,
  totalGames,
  elapsedMs,
}: ProgressSectionProps) {
  if (status === 'idle') return null;

  const pct = totalGames > 0 ? (gamesCompleted / totalGames) * 100 : 0;
  const elapsedSec = elapsedMs / 1000;
  const speed = elapsedSec > 0 ? (gamesCompleted / elapsedSec).toFixed(1) : '-';

  let eta = '-';
  if (gamesCompleted > 0 && gamesCompleted < totalGames) {
    const msPerGame = elapsedMs / gamesCompleted;
    eta = formatDuration(msPerGame * (totalGames - gamesCompleted));
  }

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <CardTitle className="text-base">Progress</CardTitle>
          <Badge variant={STATUS_VARIANT[status]}>{STATUS_LABEL[status]}</Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        <Progress value={pct} className="h-3" />
        <div className="flex items-center justify-between text-sm text-muted-foreground">
          <span>
            {gamesCompleted.toLocaleString()} / {totalGames.toLocaleString()} games ({pct.toFixed(1)}%)
          </span>
          <span className="flex gap-4">
            <span>Elapsed: {formatDuration(elapsedMs)}</span>
            {status === 'running' && <span>ETA: {eta}</span>}
            <span>{speed} games/sec</span>
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
