import { useRef } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { useCache } from '@/hooks/use-cache';
import type { GameHistory, ProgressStats, SimConfig } from '../types';

interface CachePanelProps {
  onLoad: (stats: ProgressStats, config: SimConfig, histories: GameHistory[] | null, meta: { gamesCompleted: number; totalGames: number; elapsedMs: number }) => void;
}

export default function CachePanel({ onLoad }: CachePanelProps) {
  const cache = useCache();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleLoad = (entry: typeof cache.entries[0]) => {
    const histories = entry.hasHistories ? cache.loadHistories(entry.config) : null;
    onLoad(entry.stats, entry.config, histories, {
      gamesCompleted: entry.gamesCompleted,
      totalGames: entry.totalGames,
      elapsedMs: entry.elapsedMs,
    });
  };

  const handleImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    await cache.importFile(file);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const sizeKb = cache.sizeEstimate.entries > 0
    ? (cache.sizeEstimate.used / 1024).toFixed(1)
    : null;

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <CardTitle className="text-base">Cached Simulations</CardTitle>
            {sizeKb && (
              <Badge variant="outline" className="text-xs">
                {cache.sizeEstimate.entries} entries, {sizeKb} KB
              </Badge>
            )}
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={cache.clear}>Clear</Button>
            <Button variant="outline" size="sm" onClick={() => fileInputRef.current?.click()}>
              Import
            </Button>
            <input
              ref={fileInputRef}
              type="file"
              accept=".json"
              className="hidden"
              onChange={handleImport}
            />
          </div>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        {cache.entries.length === 0 ? (
          <p className="text-sm text-muted-foreground p-4">No cached simulations</p>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Strategies</TableHead>
                <TableHead>Rules</TableHead>
                <TableHead className="text-right">Games</TableHead>
                <TableHead className="text-right">Seed</TableHead>
                <TableHead>Histories</TableHead>
                <TableHead>Saved</TableHead>
                <TableHead></TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {cache.entries.map((entry) => {
                const strats = entry.config.strategies.map((s, i) => `P${i + 1}:${s}`).join(', ');
                const timeStr = new Date(entry.savedAt).toLocaleString(undefined, {
                  month: 'short',
                  day: 'numeric',
                  hour: '2-digit',
                  minute: '2-digit',
                });

                return (
                  <TableRow key={entry.key}>
                    <TableCell className="text-xs">{strats}</TableCell>
                    <TableCell className="text-xs">{entry.config.rules}</TableCell>
                    <TableCell className="text-right text-xs">{entry.totalGames}</TableCell>
                    <TableCell className="text-right text-xs font-mono">{entry.config.seed}</TableCell>
                    <TableCell className="text-xs">{entry.hasHistories ? 'Yes' : 'No'}</TableCell>
                    <TableCell className="text-xs">{timeStr}</TableCell>
                    <TableCell>
                      <div className="flex gap-1">
                        <Button size="sm" variant="outline" className="h-6 text-xs" onClick={() => handleLoad(entry)}>
                          Load
                        </Button>
                        <Button size="sm" variant="outline" className="h-6 text-xs" onClick={() => cache.exportEntry(entry.key, entry.config)}>
                          Export
                        </Button>
                        <Button size="sm" variant="ghost" className="h-6 text-xs text-destructive" onClick={() => cache.remove(entry.key)}>
                          Delete
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  );
}
