import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import type { SimConfig, ProgressStats, CacheEntry } from '@/types';

// Mock the cache module
vi.mock('../../cache', () => ({
  getCacheEntry: vi.fn(),
  getCacheHistories: vi.fn(),
  listCacheEntries: vi.fn(() => []),
  deleteCacheEntry: vi.fn(),
  clearCache: vi.fn(),
  exportCacheEntry: vi.fn(),
  importCacheEntry: vi.fn(),
  getCacheSizeEstimate: vi.fn(() => ({ used: 0, entries: 0 })),
}));

import {
  getCacheEntry,
  getCacheHistories,
  listCacheEntries,
  deleteCacheEntry,
  clearCache,
  exportCacheEntry,
  importCacheEntry,
  getCacheSizeEstimate,
} from '../../cache';
import { useCache } from '../use-cache';

const mockedGetCacheEntry = vi.mocked(getCacheEntry);
const mockedGetCacheHistories = vi.mocked(getCacheHistories);
const mockedListCacheEntries = vi.mocked(listCacheEntries);
const mockedDeleteCacheEntry = vi.mocked(deleteCacheEntry);
const mockedClearCache = vi.mocked(clearCache);
const mockedExportCacheEntry = vi.mocked(exportCacheEntry);
const mockedImportCacheEntry = vi.mocked(importCacheEntry);
const mockedGetCacheSizeEstimate = vi.mocked(getCacheSizeEstimate);

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

function makeEntry(key: string): CacheEntry {
  return {
    version: 1,
    key,
    config: makeConfig(),
    stats: {
      num_games: 100,
      num_players: 2,
      wins_per_player: [50, 50],
      win_rate_per_player: [0.5, 0.5],
      avg_score_per_player: [30, 30],
      min_score_per_player: [5, 5],
      max_score_per_player: [80, 80],
      avg_rounds_per_game: 3,
      avg_turns_per_game: 20,
    } as ProgressStats,
    gamesCompleted: 100,
    totalGames: 100,
    elapsedMs: 500,
    hasHistories: false,
    savedAt: Date.now(),
  };
}

describe('useCache', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedListCacheEntries.mockReturnValue([]);
    mockedGetCacheSizeEstimate.mockReturnValue({ used: 0, entries: 0 });
  });

  it('returns entries and sizeEstimate from cache module', () => {
    const entries = [makeEntry('abc')];
    mockedListCacheEntries.mockReturnValue(entries);
    mockedGetCacheSizeEstimate.mockReturnValue({ used: 1024, entries: 1 });

    const { result } = renderHook(() => useCache());

    expect(result.current.entries).toEqual(entries);
    expect(result.current.sizeEstimate).toEqual({ used: 1024, entries: 1 });
  });

  it('load calls getCacheEntry', () => {
    const entry = makeEntry('abc');
    mockedGetCacheEntry.mockReturnValue(entry);

    const { result } = renderHook(() => useCache());
    const loaded = result.current.load(makeConfig());

    expect(mockedGetCacheEntry).toHaveBeenCalledWith(makeConfig());
    expect(loaded).toEqual(entry);
  });

  it('load returns null when no entry exists', () => {
    mockedGetCacheEntry.mockReturnValue(null);

    const { result } = renderHook(() => useCache());
    expect(result.current.load(makeConfig({ seed: 999 }))).toBeNull();
  });

  it('loadHistories calls getCacheHistories', () => {
    const histories = [{ rounds: [] }] as unknown as import('@/types').GameHistory[];
    mockedGetCacheHistories.mockReturnValue(histories);

    const { result } = renderHook(() => useCache());
    const loaded = result.current.loadHistories(makeConfig());

    expect(mockedGetCacheHistories).toHaveBeenCalledWith(makeConfig());
    expect(loaded).toEqual(histories);
  });

  it('remove calls deleteCacheEntry and refreshes', () => {
    const { result } = renderHook(() => useCache());
    const versionBefore = result.current.version;

    act(() => {
      result.current.remove('abc');
    });

    expect(mockedDeleteCacheEntry).toHaveBeenCalledWith('abc');
    expect(result.current.version).toBe(versionBefore + 1);
  });

  it('clear calls clearCache and refreshes', () => {
    const { result } = renderHook(() => useCache());
    const versionBefore = result.current.version;

    act(() => {
      result.current.clear();
    });

    expect(mockedClearCache).toHaveBeenCalled();
    expect(result.current.version).toBe(versionBefore + 1);
  });

  describe('exportEntry', () => {
    it('creates a download link when export data exists', () => {
      mockedExportCacheEntry.mockReturnValue('{"data":"test"}');

      const mockClick = vi.fn();
      const realCreateElement = document.createElement.bind(document);
      const mockCreateElement = vi.spyOn(document, 'createElement').mockImplementation((tag: string, options?: ElementCreationOptions) => {
        if (tag === 'a') {
          const anchor = realCreateElement('a', options);
          anchor.click = mockClick;
          return anchor;
        }
        return realCreateElement(tag, options);
      });

      const mockCreateObjectURL = vi.fn(() => 'blob:url');
      const mockRevokeObjectURL = vi.fn();
      const realURL = globalThis.URL;
      vi.stubGlobal('URL', Object.assign((...args: ConstructorParameters<typeof URL>) => new realURL(...args), { createObjectURL: mockCreateObjectURL, revokeObjectURL: mockRevokeObjectURL }));

      const { result } = renderHook(() => useCache());

      act(() => {
        result.current.exportEntry('abc', makeConfig({ seed: 42, num_games: 100 }));
      });

      expect(mockedExportCacheEntry).toHaveBeenCalledWith('abc');
      expect(mockClick).toHaveBeenCalled();

      mockCreateElement.mockRestore();
      vi.unstubAllGlobals();
    });

    it('does nothing when exportCacheEntry returns null', () => {
      mockedExportCacheEntry.mockReturnValue(null);

      const mockCreateObjectURL = vi.fn();
      const realURL = globalThis.URL;
      vi.stubGlobal('URL', Object.assign((...args: ConstructorParameters<typeof URL>) => new realURL(...args), { createObjectURL: mockCreateObjectURL, revokeObjectURL: vi.fn() }));

      const { result } = renderHook(() => useCache());

      act(() => {
        result.current.exportEntry('abc', makeConfig());
      });

      expect(mockCreateObjectURL).not.toHaveBeenCalled();

      vi.unstubAllGlobals();
    });
  });

  describe('importFile', () => {
    it('resolves true on successful import', async () => {
      mockedImportCacheEntry.mockReturnValue('key123');

      const fileContent = '{"format":"skyjo-sim-cache","version":1}';
      const file = new File([fileContent], 'test.json', { type: 'application/json' });

      const { result } = renderHook(() => useCache());

      let importResult: boolean | undefined;
      await act(async () => {
        importResult = await result.current.importFile(file);
      });

      expect(importResult).toBe(true);
      expect(mockedImportCacheEntry).toHaveBeenCalledWith(fileContent);
    });

    it('resolves false when importCacheEntry returns null', async () => {
      mockedImportCacheEntry.mockReturnValue(null);

      const file = new File(['bad data'], 'test.json', { type: 'application/json' });

      const { result } = renderHook(() => useCache());

      let importResult: boolean | undefined;
      await act(async () => {
        importResult = await result.current.importFile(file);
      });

      expect(importResult).toBe(false);
    });
  });

  it('refresh increments version', () => {
    const { result } = renderHook(() => useCache());
    const v0 = result.current.version;

    act(() => {
      result.current.refresh();
    });

    expect(result.current.version).toBe(v0 + 1);
  });
});
