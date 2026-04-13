import { useState, useCallback } from 'react';
import type { CacheEntry, GameHistory, ProgressStats, SimConfig } from '../types';
import {
  getCacheEntry,
  getCacheHistories,
  listCacheEntries,
  deleteCacheEntry,
  clearCache,
  exportCacheEntry,
  importCacheEntry,
  getCacheSizeEstimate,
} from '../cache';

export function useCache() {
  const [version, setVersion] = useState(0);

  const refresh = useCallback(() => setVersion((v) => v + 1), []);

  const entries = listCacheEntries();
  const sizeEstimate = getCacheSizeEstimate();

  const load = useCallback((config: SimConfig): CacheEntry | null => {
    return getCacheEntry(config);
  }, []);

  const loadHistories = useCallback((config: SimConfig): GameHistory[] | null => {
    return getCacheHistories(config);
  }, []);

  const remove = useCallback((key: string) => {
    deleteCacheEntry(key);
    refresh();
  }, [refresh]);

  const clear = useCallback(() => {
    clearCache();
    refresh();
  }, [refresh]);

  const exportEntry = useCallback((key: string, config: SimConfig) => {
    const json = exportCacheEntry(key);
    if (!json) return;
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `skyjo-sim-${config.seed}-${config.num_games}g.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, []);

  const importFile = useCallback((file: File): Promise<boolean> => {
    return new Promise((resolve) => {
      const reader = new FileReader();
      reader.onload = () => {
        const json = reader.result as string;
        const result = importCacheEntry(json);
        if (result) {
          refresh();
          resolve(true);
        } else {
          resolve(false);
        }
      };
      reader.onerror = () => resolve(false);
      reader.readAsText(file);
    });
  }, [refresh]);

  return {
    entries,
    sizeEstimate,
    load,
    loadHistories,
    remove,
    clear,
    exportEntry,
    importFile,
    version,
    refresh,
  };
}
