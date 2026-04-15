import { useState, useEffect, useCallback } from 'react';
import { useAuth } from '@/contexts/auth-context';
import { GameDetailSchema, GameHistorySchema } from '@/schemas';
import type { GameDetail, GameHistory } from '@/types';

export function useGameDetail(gameId: string) {
  const { accessToken } = useAuth();

  const [game, setGame] = useState<GameDetail | null>(null);
  const [replay, setReplay] = useState<GameHistory | null>(null);
  const [loading, setLoading] = useState(true);
  const [replayLoading, setReplayLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const authHeaders = useCallback((): Record<string, string> => {
    const headers: Record<string, string> = {};
    if (accessToken) {
      headers['Authorization'] = `Bearer ${accessToken}`;
    }
    return headers;
  }, [accessToken]);

  useEffect(() => {
    let cancelled = false;

    async function fetchGame() {
      setLoading(true);
      setError(null);
      setReplay(null);

      try {
        const res = await fetch(`/api/games/${gameId}`, {
          headers: authHeaders(),
        });
        if (!res.ok) {
          const body = await res.json().catch(() => ({}));
          throw new Error(body?.error?.message ?? `Failed to load game (${res.status})`);
        }

        const data = await res.json();
        const parsed = GameDetailSchema.parse(data);
        if (!cancelled) {
          setGame(parsed);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load game');
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    fetchGame();
    return () => {
      cancelled = true;
    };
  }, [gameId, authHeaders]);

  const loadReplay = useCallback(async () => {
    if (replay || replayLoading) return;

    setReplayLoading(true);
    try {
      const res = await fetch(`/api/games/${gameId}/replay`, {
        headers: authHeaders(),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message ?? `Failed to load replay (${res.status})`);
      }

      const data = await res.json();
      const parsed = GameHistorySchema.parse(data);
      setReplay(parsed);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load replay');
    } finally {
      setReplayLoading(false);
    }
  }, [gameId, replay, replayLoading, authHeaders]);

  return {
    game,
    replay,
    loadReplay,
    loading,
    replayLoading,
    error,
  };
}
