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
import type { ProgressStats } from '../types';

interface StatsTableProps {
  stats: ProgressStats;
  strategyNames: string[];
  gamesCompleted: number;
  onExport?: () => void;
}

export default function StatsTable({ stats, strategyNames, gamesCompleted, onExport }: StatsTableProps) {
  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle>Results</CardTitle>
          {onExport && (
            <Button variant="outline" size="sm" onClick={onExport}>
              Export Result
            </Button>
          )}
        </div>
        <div className="text-sm text-muted-foreground flex gap-4 flex-wrap">
          <span>Games: {gamesCompleted.toLocaleString()}</span>
          <span>Avg rounds/game: {stats.avg_rounds_per_game.toFixed(2)}</span>
          <span>Avg turns/game: {stats.avg_turns_per_game.toFixed(1)}</span>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Player</TableHead>
              <TableHead className="text-right">Wins</TableHead>
              <TableHead className="text-right">Win Rate</TableHead>
              <TableHead className="text-right">Avg Score</TableHead>
              <TableHead className="text-right">Min Score</TableHead>
              <TableHead className="text-right">Max Score</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {Array.from({ length: stats.num_players }, (_, p) => (
              <TableRow key={p}>
                <TableCell className="font-medium">
                  Player {p + 1} ({strategyNames[p] ?? ''})
                </TableCell>
                <TableCell className="text-right">{stats.wins_per_player[p]}</TableCell>
                <TableCell className="text-right">
                  {(stats.win_rate_per_player[p] * 100).toFixed(1)}%
                </TableCell>
                <TableCell className="text-right">
                  {stats.avg_score_per_player[p].toFixed(1)}
                </TableCell>
                <TableCell className="text-right">{stats.min_score_per_player[p]}</TableCell>
                <TableCell className="text-right">{stats.max_score_per_player[p]}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
