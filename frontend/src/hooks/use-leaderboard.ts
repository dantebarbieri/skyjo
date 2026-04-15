import { useState, useEffect, useCallback, useRef } from 'react';
import { useAuth } from '@/contexts/auth-context';
import { GameListResponseSchema } from '@/schemas';
import type { GameSummary } from '@/types';

export type SortBy = 'created_at' | 'num_rounds' | 'num_players';
export type SortOrder = 'asc' | 'desc';

export interface LeaderboardFilters {
  playerName: string;
  myGames: boolean;
  numPlayers: number | null;
  rules: string | null;
}

export function useLeaderboard() {
  const { accessToken, user } = useAuth();

  const [games, setGames] = useState<GameSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [perPage, setPerPage] = useState(25);
  const [sortBy, setSortBy] = useState<SortBy>('created_at');
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc');
  const [filters, setFilters] = useState<LeaderboardFilters>({
    playerName: '',
    myGames: false,
    numPlayers: null,
    rules: null,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Debounce player name search
  const [debouncedPlayerName, setDebouncedPlayerName] = useState('');
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedPlayerName(filters.playerName);
      setPage(1);
    }, 300);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [filters.playerName]);

  const fetchGames = useCallback(async () => {
    setLoading(true);
    setError(null);

    const params = new URLSearchParams();
    params.set('page', String(page));
    params.set('per_page', String(perPage));
    params.set('sort_by', sortBy);
    params.set('sort_order', sortOrder);

    if (debouncedPlayerName.trim()) {
      params.set('player_name', debouncedPlayerName.trim());
    }
    if (filters.myGames && user?.id) {
      params.set('user_id', user.id);
    }
    if (filters.numPlayers !== null) {
      params.set('min_players', String(filters.numPlayers));
      params.set('max_players', String(filters.numPlayers));
    }
    if (filters.rules) {
      params.set('rules', filters.rules);
    }

    try {
      const headers: Record<string, string> = {};
      if (accessToken) {
        headers['Authorization'] = `Bearer ${accessToken}`;
      }

      const res = await fetch(`/api/games?${params.toString()}`, { headers });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message ?? `Failed to load games (${res.status})`);
      }

      const data = await res.json();
      const parsed = GameListResponseSchema.parse(data);
      setGames(parsed.games);
      setTotal(parsed.total);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load games');
      setGames([]);
      setTotal(0);
    } finally {
      setLoading(false);
    }
  }, [page, perPage, sortBy, sortOrder, debouncedPlayerName, filters.myGames, filters.numPlayers, filters.rules, accessToken, user?.id]);

  useEffect(() => {
    fetchGames();
  }, [fetchGames]);

  const setSort = useCallback((field: SortBy) => {
    setSortBy((prev) => {
      if (prev === field) {
        setSortOrder((o) => (o === 'asc' ? 'desc' : 'asc'));
      } else {
        setSortOrder('desc');
      }
      return field;
    });
    setPage(1);
  }, []);

  const updateFilters = useCallback((update: Partial<LeaderboardFilters>) => {
    setFilters((prev) => ({ ...prev, ...update }));
    // Reset page for non-playerName changes (playerName resets via debounce)
    if (!('playerName' in update)) {
      setPage(1);
    }
  }, []);

  const updatePerPage = useCallback((newPerPage: number) => {
    setPerPage(newPerPage);
    setPage(1);
  }, []);

  return {
    games,
    total,
    page,
    perPage,
    sortBy,
    sortOrder,
    filters,
    loading,
    error,
    setPage,
    setPerPage: updatePerPage,
    setSort,
    setFilters: updateFilters,
    refresh: fetchGames,
  };
}
