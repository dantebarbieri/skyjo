import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from '@/components/ui/pagination';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import ScoringSheet from './scoring-sheet';
import type { GameHistory } from '../types';

interface GameListProps {
  histories: GameHistory[];
  onReplay: (history: GameHistory, index: number) => void;
  selectedIndex: number | null;
}

const PAGE_SIZES = [25, 50, 100];

export default function GameList({ histories, onReplay, selectedIndex }: GameListProps) {
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(25);
  const [scoringGameIndex, setScoringGameIndex] = useState<number | null>(null);

  const totalPages = Math.ceil(histories.length / pageSize);
  const pageHistories = useMemo(
    () => histories.slice(page * pageSize, (page + 1) * pageSize),
    [histories, page, pageSize]
  );

  const handlePageChange = (newPage: number) => {
    setPage(Math.max(0, Math.min(newPage, totalPages - 1)));
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle>Game Histories</CardTitle>
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <span>{histories.length} games</span>
            <Select value={String(pageSize)} onValueChange={(v) => { setPageSize(parseInt(v)); setPage(0); }}>
              <SelectTrigger className="w-20 h-8">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PAGE_SIZES.map((s) => (
                  <SelectItem key={s} value={String(s)}>{s}/page</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12">#</TableHead>
              <TableHead>Seed</TableHead>
              <TableHead className="text-right">Rounds</TableHead>
              <TableHead className="text-right">Turns</TableHead>
              <TableHead className="text-right">Col Clears</TableHead>
              <TableHead>Winner</TableHead>
              <TableHead>Scores</TableHead>
              <TableHead className="w-32"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {pageHistories.map((h, localIdx) => {
              const globalIdx = page * pageSize + localIdx;
              const totalTurns = h.rounds.reduce((sum, r) => sum + r.turns.length, 0);
              const totalClears = h.rounds.reduce(
                (sum, r) =>
                  sum +
                  r.end_of_round_clears.length +
                  r.turns.reduce((ts, t) => ts + t.column_clears.length, 0),
                0
              );
              const wasTruncated = h.rounds.some((r) => r.truncated);
              const winnerStr = h.winners.map((w) => `P${w + 1}`).join(', ');

              return (
                <TableRow
                  key={globalIdx}
                  className={selectedIndex === globalIdx ? 'bg-accent' : ''}
                >
                  <TableCell className="font-mono text-sm">{globalIdx + 1}</TableCell>
                  <TableCell className="font-mono text-sm">{h.seed}</TableCell>
                  <TableCell className="text-right">{h.rounds.length}</TableCell>
                  <TableCell className="text-right">
                    {totalTurns}
                    {wasTruncated && (
                      <Badge variant="destructive" className="ml-1 text-[10px] px-1 py-0">!</Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right">{totalClears}</TableCell>
                  <TableCell>{winnerStr}</TableCell>
                  <TableCell>
                    {h.final_scores.map((s, p) => (
                      <span key={p}>
                        {p > 0 && ', '}
                        <span className={h.winners.includes(p) ? 'font-bold' : ''}>
                          {s}
                        </span>
                      </span>
                    ))}
                  </TableCell>
                  <TableCell>
                    <div className="flex gap-1">
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 text-xs"
                        onClick={() => onReplay(h, globalIdx)}
                      >
                        Replay
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 text-xs"
                        onClick={() => setScoringGameIndex(scoringGameIndex === globalIdx ? null : globalIdx)}
                      >
                        Scores
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>

        {scoringGameIndex !== null && histories[scoringGameIndex] && (
          <div className="border-t p-4">
            <ScoringSheet
              history={histories[scoringGameIndex]}
              onClose={() => setScoringGameIndex(null)}
            />
          </div>
        )}

        {totalPages > 1 && (
          <div className="flex justify-center py-4">
            <Pagination>
              <PaginationContent>
                <PaginationItem>
                  <PaginationPrevious
                    onClick={() => handlePageChange(page - 1)}
                    className={page === 0 ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                  />
                </PaginationItem>
                {Array.from({ length: Math.min(totalPages, 7) }, (_, i) => {
                  let pageIdx: number;
                  if (totalPages <= 7) {
                    pageIdx = i;
                  } else if (page < 3) {
                    pageIdx = i;
                  } else if (page > totalPages - 4) {
                    pageIdx = totalPages - 7 + i;
                  } else {
                    pageIdx = page - 3 + i;
                  }
                  return (
                    <PaginationItem key={pageIdx}>
                      <PaginationLink
                        isActive={pageIdx === page}
                        onClick={() => handlePageChange(pageIdx)}
                        className="cursor-pointer"
                      >
                        {pageIdx + 1}
                      </PaginationLink>
                    </PaginationItem>
                  );
                })}
                <PaginationItem>
                  <PaginationNext
                    onClick={() => handlePageChange(page + 1)}
                    className={page >= totalPages - 1 ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                  />
                </PaginationItem>
              </PaginationContent>
            </Pagination>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
