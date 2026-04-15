import { useNavigate } from 'react-router-dom';
import { useAuth } from '@/contexts/auth-context';
import { useLeaderboard, type SortBy } from '@/hooks/use-leaderboard';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
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
import { Checkbox } from '@/components/ui/checkbox';
import { Trophy } from 'lucide-react';
import type { GameSummary } from '@/types';

const PAGE_SIZES = [25, 50, 100];

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function SkeletonRow({ cols }: { cols: number }) {
  return (
    <TableRow>
      {Array.from({ length: cols }, (_, i) => (
        <TableCell key={i}>
          <div className="h-4 bg-muted rounded animate-pulse" />
        </TableCell>
      ))}
    </TableRow>
  );
}

function ScoreBadges({ game }: { game: GameSummary }) {
  return (
    <div className="flex flex-wrap gap-1">
      {game.players.map((p, i) => (
        <span
          key={i}
          className={cn(
            'text-xs',
            p.is_winner && 'font-bold text-green-600 dark:text-green-400',
          )}
        >
          {i > 0 && ', '}
          {p.final_score}
          {p.is_winner && (
            <span className="text-[10px] ml-0.5">W</span>
          )}
        </span>
      ))}
    </div>
  );
}

export default function LeaderboardRoute() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const lb = useLeaderboard();

  const sortIndicator = (field: SortBy) =>
    lb.sortBy === field ? (lb.sortOrder === 'asc' ? ' ▲' : ' ▼') : '';

  const totalPages = Math.max(1, Math.ceil(lb.total / lb.perPage));

  const handlePageChange = (newPage: number) => {
    lb.setPage(Math.max(1, Math.min(newPage, totalPages)));
  };

  const showYourScore = user !== null;
  const colCount = showYourScore ? 7 : 6;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Trophy className="h-6 w-6" />
        <h1 className="text-2xl font-bold">Leaderboard</h1>
      </div>

      <Card>
        <CardHeader>
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
            <CardTitle>Game History</CardTitle>
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span>{lb.total} games</span>
              <Select
                value={String(lb.perPage)}
                onValueChange={(v) => lb.setPerPage(parseInt(v))}
              >
                <SelectTrigger className="w-20 h-8">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PAGE_SIZES.map((s) => (
                    <SelectItem key={s} value={String(s)}>
                      {s}/page
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {/* Filters */}
          <div className="flex gap-2 items-center flex-wrap mt-2">
            <Input
              placeholder="Search player..."
              value={lb.filters.playerName}
              onChange={(e) => lb.setFilters({ playerName: e.target.value })}
              className="w-32 sm:w-40 h-8 text-sm"
            />
            <Select
              value={lb.filters.numPlayers !== null ? String(lb.filters.numPlayers) : 'all'}
              onValueChange={(v) =>
                lb.setFilters({ numPlayers: v === 'all' ? null : parseInt(v) })
              }
            >
              <SelectTrigger className="w-28 h-8">
                <SelectValue placeholder="Players" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All players</SelectItem>
                {[2, 3, 4, 5, 6, 7, 8].map((n) => (
                  <SelectItem key={n} value={String(n)}>
                    {n} players
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select
              value={lb.filters.rules ?? 'all'}
              onValueChange={(v) =>
                lb.setFilters({ rules: v === 'all' ? null : v })
              }
            >
              <SelectTrigger className="w-28 h-8">
                <SelectValue placeholder="Rules" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All rules</SelectItem>
                <SelectItem value="Standard">Standard</SelectItem>
              </SelectContent>
            </Select>
            {user && (
              <div className="flex items-center gap-1.5">
                <Checkbox
                  id="my-games"
                  checked={lb.filters.myGames}
                  onCheckedChange={(checked) =>
                    lb.setFilters({ myGames: checked === true })
                  }
                />
                <label htmlFor="my-games" className="text-sm cursor-pointer">
                  My Games
                </label>
              </div>
            )}
          </div>
        </CardHeader>

        <CardContent className="p-0">
          {lb.error && (
            <div className="px-4 py-3 text-sm text-destructive">{lb.error}</div>
          )}

          <Table>
            <TableHeader>
              <TableRow>
                <TableHead
                  className="cursor-pointer select-none"
                  onClick={() => lb.setSort('created_at')}
                >
                  Date{sortIndicator('created_at')}
                </TableHead>
                <TableHead
                  className="text-center cursor-pointer select-none"
                  onClick={() => lb.setSort('num_players')}
                >
                  Players{sortIndicator('num_players')}
                </TableHead>
                <TableHead>Player Names</TableHead>
                <TableHead>Final Scores</TableHead>
                <TableHead
                  className="text-right cursor-pointer select-none"
                  onClick={() => lb.setSort('num_rounds')}
                >
                  Rounds{sortIndicator('num_rounds')}
                </TableHead>
                {showYourScore && (
                  <TableHead className="text-right">Your Score</TableHead>
                )}
                <TableHead className="w-20" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {lb.loading &&
                Array.from({ length: 5 }, (_, i) => (
                  <SkeletonRow key={i} cols={colCount} />
                ))}
              {!lb.loading && lb.games.length === 0 && (
                <TableRow>
                  <TableCell
                    colSpan={colCount}
                    className="text-center py-8 text-muted-foreground"
                  >
                    No games found
                  </TableCell>
                </TableRow>
              )}
              {!lb.loading &&
                lb.games.map((game) => (
                  <TableRow key={game.id}>
                    <TableCell className="text-sm">
                      {formatDate(game.created_at)}
                    </TableCell>
                    <TableCell className="text-center">
                      <Badge variant="secondary">{game.num_players}</Badge>
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-1">
                        {game.players.map((p, i) => (
                          <span key={i} className="text-xs">
                            {i > 0 && ', '}
                            {p.name}
                            {p.is_bot && (
                              <span className="text-muted-foreground ml-0.5">
                                (bot)
                              </span>
                            )}
                          </span>
                        ))}
                      </div>
                    </TableCell>
                    <TableCell>
                      <ScoreBadges game={game} />
                    </TableCell>
                    <TableCell className="text-right">
                      {game.num_rounds}
                    </TableCell>
                    {showYourScore && (
                      <TableCell className="text-right font-mono text-sm">
                        {game.your_score != null ? game.your_score : '—'}
                      </TableCell>
                    )}
                    <TableCell>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 text-xs"
                        onClick={() =>
                          navigate(`/leaderboard/${game.id}`)
                        }
                      >
                        View
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
            </TableBody>
          </Table>

          <div className="px-4 py-2 text-[10px] text-muted-foreground border-t">
            <span className="text-green-600 font-bold">Green bold</span> = winner |{' '}
            <span className="text-[10px]">W</span> = winner
          </div>

          {totalPages > 1 && (
            <div className="flex justify-center py-4">
              <Pagination>
                <PaginationContent>
                  <PaginationItem>
                    <PaginationPrevious
                      onClick={() => handlePageChange(lb.page - 1)}
                      className={
                        lb.page <= 1
                          ? 'pointer-events-none opacity-50'
                          : 'cursor-pointer'
                      }
                    />
                  </PaginationItem>
                  {Array.from(
                    { length: Math.min(totalPages, 7) },
                    (_, i) => {
                      let pageIdx: number;
                      if (totalPages <= 7) {
                        pageIdx = i + 1;
                      } else if (lb.page < 4) {
                        pageIdx = i + 1;
                      } else if (lb.page > totalPages - 3) {
                        pageIdx = totalPages - 6 + i;
                      } else {
                        pageIdx = lb.page - 3 + i;
                      }
                      return (
                        <PaginationItem key={pageIdx}>
                          <PaginationLink
                            isActive={pageIdx === lb.page}
                            onClick={() => handlePageChange(pageIdx)}
                            className="cursor-pointer"
                          >
                            {pageIdx}
                          </PaginationLink>
                        </PaginationItem>
                      );
                    },
                  )}
                  <PaginationItem>
                    <PaginationNext
                      onClick={() => handlePageChange(lb.page + 1)}
                      className={
                        lb.page >= totalPages
                          ? 'pointer-events-none opacity-50'
                          : 'cursor-pointer'
                      }
                    />
                  </PaginationItem>
                </PaginationContent>
              </Pagination>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
