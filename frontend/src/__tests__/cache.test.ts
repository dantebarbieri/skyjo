import {
  saveCacheEntry,
  getCacheEntry,
  getCacheHistories,
  listCacheEntries,
  deleteCacheEntry,
  clearCache,
  exportCacheEntry,
  importCacheEntry,
  getCacheSizeEstimate,
} from '@/cache';
import type { ProgressStats, SimConfig } from '@/types';

function makeConfig(overrides: Partial<SimConfig> = {}): SimConfig {
  return {
    num_games: 100,
    seed: 42,
    strategies: ['Random', 'Random'],
    rules: 'Standard',
    withHistories: false,
    realtimeVisualization: false,
    maxTurnsPerRound: 50,
    ...overrides,
  };
}

function makeStats(overrides: Partial<ProgressStats> = {}): ProgressStats {
  return {
    num_games: 100,
    num_players: 2,
    wins_per_player: [55, 45],
    win_rate_per_player: [0.55, 0.45],
    avg_score_per_player: [30.5, 35.2],
    min_score_per_player: [5, 8],
    max_score_per_player: [80, 90],
    avg_rounds_per_game: 3.2,
    avg_turns_per_game: 24.5,
    ...overrides,
  };
}

const META = { elapsedMs: 500, gamesCompleted: 100, totalGames: 100 };

beforeEach(() => {
  localStorage.clear();
});

describe('saveCacheEntry / getCacheEntry round-trip', () => {
  it('stores and retrieves a cache entry', () => {
    const config = makeConfig();
    const stats = makeStats();
    saveCacheEntry(config, stats, META, null);

    const entry = getCacheEntry(config);
    expect(entry).not.toBeNull();
    expect(entry!.version).toBe(1);
    expect(entry!.config).toEqual(config);
    expect(entry!.stats).toEqual(stats);
    expect(entry!.gamesCompleted).toBe(100);
    expect(entry!.hasHistories).toBe(false);
  });

  it('returns null for a config that was never saved', () => {
    expect(getCacheEntry(makeConfig({ seed: 999 }))).toBeNull();
  });
});

describe('getCacheHistories', () => {
  it('returns stored histories', () => {
    const config = makeConfig();
    const histories = [{ turns: [1, 2, 3] }] as unknown as import('@/types').GameHistory[];
    saveCacheEntry(config, makeStats(), META, histories);

    const result = getCacheHistories(config);
    expect(result).toEqual(histories);
  });

  it('returns null when no histories were saved', () => {
    const config = makeConfig();
    saveCacheEntry(config, makeStats(), META, null);
    expect(getCacheHistories(config)).toBeNull();
  });
});

describe('listCacheEntries', () => {
  it('lists all saved entries in most-recent-first order', () => {
    const configA = makeConfig({ seed: 1 });
    const configB = makeConfig({ seed: 2 });
    saveCacheEntry(configA, makeStats(), META, null);
    saveCacheEntry(configB, makeStats(), META, null);

    const entries = listCacheEntries();
    expect(entries).toHaveLength(2);
    // Most recently saved (configB) should come first
    expect(entries[0].config.seed).toBe(2);
    expect(entries[1].config.seed).toBe(1);
  });
});

describe('deleteCacheEntry', () => {
  it('removes a specific entry by key', () => {
    const config = makeConfig();
    saveCacheEntry(config, makeStats(), META, null);
    const entry = getCacheEntry(config)!;

    deleteCacheEntry(entry.key);
    expect(getCacheEntry(config)).toBeNull();
    expect(listCacheEntries()).toHaveLength(0);
  });
});

describe('clearCache', () => {
  it('removes all entries', () => {
    saveCacheEntry(makeConfig({ seed: 1 }), makeStats(), META, null);
    saveCacheEntry(makeConfig({ seed: 2 }), makeStats(), META, null);
    expect(listCacheEntries()).toHaveLength(2);

    clearCache();
    expect(listCacheEntries()).toHaveLength(0);
  });
});

describe('exportCacheEntry / importCacheEntry round-trip', () => {
  it('exports and re-imports an entry', () => {
    const config = makeConfig();
    const stats = makeStats();
    const histories = [{ data: 'game1' }] as unknown as import('@/types').GameHistory[];
    saveCacheEntry(config, stats, META, histories);

    const entry = getCacheEntry(config)!;
    const json = exportCacheEntry(entry.key)!;
    expect(json).toBeTruthy();

    clearCache();
    expect(listCacheEntries()).toHaveLength(0);

    const importedKey = importCacheEntry(json);
    expect(importedKey).toBeTruthy();

    const restored = getCacheEntry(config);
    expect(restored).not.toBeNull();
    expect(restored!.stats).toEqual(stats);
  });

  it('returns null for invalid JSON on import', () => {
    expect(importCacheEntry('not-json')).toBeNull();
  });
});

describe('getCacheSizeEstimate', () => {
  it('returns used bytes and entry count', () => {
    expect(getCacheSizeEstimate()).toEqual({ used: 0, entries: 0 });

    saveCacheEntry(makeConfig(), makeStats(), META, null);
    const estimate = getCacheSizeEstimate();
    expect(estimate.entries).toBe(1);
    expect(estimate.used).toBeGreaterThan(0);
  });
});

describe('deterministic cache key', () => {
  it('same config always maps to the same entry', () => {
    const config = makeConfig();
    saveCacheEntry(config, makeStats(), META, null);
    const entry1 = getCacheEntry(config)!;
    // Re-fetch with an identical (but separately constructed) config
    const entry2 = getCacheEntry(makeConfig())!;
    expect(entry1.key).toBe(entry2.key);
  });
});
