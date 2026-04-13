import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
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
import { cn } from '@/lib/utils';
import type { GameHistory } from '../types';

type SortField = 'index' | 'rounds' | 'turns' | 'clears' | 'score';
type SortDir = 'asc' | 'desc';

interface GameListProps {
  histories: GameHistory[];
  onView: (history: GameHistory, index: number) => void;
  selectedIndex: number | null;
}

const PAGE_SIZES = [25, 50, 100];

export default function GameList({ histories, onView, selectedIndex }: GameListProps) {
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(25);
  const [seedFilter, setSeedFilter] = useState('');
  const [winnerFilter, setWinnerFilter] = useState<string>('all');
  const [sortField, setSortField] = useState<SortField>('index');
  const [sortDir, setSortDir] = useState<SortDir>('asc');

  const filteredHistories = useMemo(() => {
    let result = histories.map((h, i) => ({ history: h, originalIndex: i }));

    if (seedFilter.trim()) {
      const seedVal = parseInt(seedFilter);
      if (!isNaN(seedVal)) {
        result = result.filter(({ history }) => history.seed === seedVal);
      }
    }

    if (winnerFilter !== 'all') {
      const w = parseInt(winnerFilter);
      result = result.filter(({ history }) => history.winners.includes(w));
    }

    if (sortField !== 'index') {
      result.sort((a, b) => {
        let cmp = 0;
        switch (sortField) {
          case 'rounds':
            cmp = a.history.rounds.length - b.history.rounds.length;
            break;
          case 'turns': {
            const turnsA = a.history.rounds.reduce((s, r) => s + r.turns.length, 0);
            const turnsB = b.history.rounds.reduce((s, r) => s + r.turns.length, 0);
            cmp = turnsA - turnsB;
            break;
          }
          case 'clears': {
            const clearsA = a.history.rounds.reduce((s, r) => s + r.end_of_round_clears.length + r.turns.reduce((ts, t) => ts + t.column_clears.length, 0), 0);
            const clearsB = b.history.rounds.reduce((s, r) => s + r.end_of_round_clears.length + r.turns.reduce((ts, t) => ts + t.column_clears.length, 0), 0);
            cmp = clearsA - clearsB;
            break;
          }
          case 'score':
            cmp = Math.min(...a.history.final_scores) - Math.min(...b.history.final_scores);
            break;
        }
        return sortDir === 'desc' ? -cmp : cmp;
      });
    } else if (sortDir === 'desc') {
      result.reverse();
    }

    return result;
  }, [histories, seedFilter, winnerFilter, sortField, sortDir]);

  const totalPages = Math.ceil(filteredHistories.length / pageSize);
  const pageItems = useMemo(
    () => filteredHistories.slice(page * pageSize, (page + 1) * pageSize),
    [filteredHistories, page, pageSize]
  );

  const toggleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortField(field);
      setSortDir('asc');
    }
    setPage(0);
  };

  const sortIndicator = (field: SortField) =>
    sortField === field ? (sortDir === 'asc' ? ' \u25B2' : ' \u25BC') : '';

  const handlePageChange = (newPage: number) => {
    setPage(Math.max(0, Math.min(newPage, totalPages - 1)));
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
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
        <div className="flex gap-2 items-center flex-wrap mt-2">
          <Input
            placeholder="Search seed..."
            value={seedFilter}
            onChange={(e) => { setSeedFilter(e.target.value); setPage(0); }}
            className="w-24 sm:w-32 h-8 text-sm"
          />
          <Select value={winnerFilter} onValueChange={(v) => { setWinnerFilter(v); setPage(0); }}>
            <SelectTrigger className="w-24 sm:w-32 h-8">
              <SelectValue placeholder="Winner" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All winners</SelectItem>
              {Array.from({ length: histories[0]?.num_players ?? 0 }, (_, i) => (
                <SelectItem key={i} value={String(i)}>P{i + 1}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          {(seedFilter || winnerFilter !== 'all') && (
            <span className="text-xs text-muted-foreground">
              {filteredHistories.length} of {histories.length} games
            </span>
          )}
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12 cursor-pointer select-none" onClick={() => toggleSort('index')}>#{sortIndicator('index')}</TableHead>
              <TableHead className="hidden sm:table-cell">Seed</TableHead>
              <TableHead className="text-right cursor-pointer select-none" onClick={() => toggleSort('rounds')}>Rounds{sortIndicator('rounds')}</TableHead>
              <TableHead className="text-right cursor-pointer select-none" onClick={() => toggleSort('turns')}>Turns{sortIndicator('turns')}</TableHead>
              <TableHead className="text-right cursor-pointer select-none hidden md:table-cell" onClick={() => toggleSort('clears')}>Col Clears{sortIndicator('clears')}</TableHead>
              <TableHead>Winner</TableHead>
              <TableHead className="cursor-pointer select-none hidden md:table-cell" onClick={() => toggleSort('score')}>Scores{sortIndicator('score')}</TableHead>
              <TableHead className="w-20"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {pageItems.map(({ history: h, originalIndex }) => {
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
              const isSingleRound = h.rounds.length === 1;

              return (
                <TableRow
                  key={originalIndex}
                  className={selectedIndex === originalIndex ? 'bg-accent' : ''}
                >
                  <TableCell className="font-mono text-sm">{originalIndex + 1}</TableCell>
                  <TableCell className="font-mono text-sm hidden sm:table-cell">{h.seed}</TableCell>
                  <TableCell className="text-right">{h.rounds.length}</TableCell>
                  <TableCell className="text-right">
                    {totalTurns}
                    {wasTruncated && (
                      <Badge variant="destructive" className="ml-1 text-[10px] px-1 py-0">!</Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right hidden md:table-cell">{totalClears}</TableCell>
                  <TableCell>{winnerStr}</TableCell>
                  <TableCell className="hidden md:table-cell">
                    {h.final_scores.map((s, p) => {
                      const isGoingOut = isSingleRound && h.rounds[0].going_out_player === p;
                      return (
                        <span key={p}>
                          {p > 0 && ', '}
                          <span className={cn(
                            h.winners.includes(p) && 'font-bold',
                            isGoingOut && 'underline underline-offset-2',
                          )}>
                            {s}
                          </span>
                        </span>
                      );
                    })}
                  </TableCell>
                  <TableCell>
                    <Button
                      size="sm"
                      variant={selectedIndex === originalIndex ? 'default' : 'outline'}
                      className="h-7 text-xs"
                      onClick={() => onView(h, originalIndex)}
                    >
                      View
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>

        <div className="px-4 py-2 text-[10px] text-muted-foreground border-t">
          <strong>Bold</strong> = winner | <span className="underline underline-offset-2">Underline</span> = went out (single-round games)
        </div>

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
