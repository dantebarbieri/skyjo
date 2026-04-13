import type { CacheEntry, CacheExportFile, GameHistory, ProgressStats, SimConfig } from './types';

const INDEX_KEY = 'skyjo_cache_index';
const SIM_PREFIX = 'skyjo_sim_';
const HIST_PREFIX = 'skyjo_hist_';

function djb2Hash(str: string): string {
  let hash = 5381;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) + hash + str.charCodeAt(i)) >>> 0;
  }
  return hash.toString(36);
}

function computeCacheKey(config: SimConfig): string {
  const obj = {
    strategies: config.strategies,
    rules: config.rules,
    seed: config.seed,
    num_games: config.num_games,
    maxTurnsPerRound: config.maxTurnsPerRound,
  };
  return djb2Hash(JSON.stringify(obj));
}

function getIndex(): string[] {
  try {
    const raw = localStorage.getItem(INDEX_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function setIndex(index: string[]): void {
  localStorage.setItem(INDEX_KEY, JSON.stringify(index));
}

function touchIndex(key: string): void {
  const index = getIndex().filter(k => k !== key);
  index.unshift(key);
  setIndex(index);
}

export function getCacheEntry(config: SimConfig): CacheEntry | null {
  const key = computeCacheKey(config);
  try {
    const raw = localStorage.getItem(SIM_PREFIX + key);
    if (!raw) return null;
    const entry: CacheEntry = JSON.parse(raw);
    if (entry.version !== 1) return null;
    touchIndex(key);
    return entry;
  } catch {
    return null;
  }
}

export function getCacheHistories(config: SimConfig): GameHistory[] | null {
  const key = computeCacheKey(config);
  try {
    const raw = localStorage.getItem(HIST_PREFIX + key);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

function trySetItem(key: string, value: string): boolean {
  try {
    localStorage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

function evictOldest(): boolean {
  const index = getIndex();
  if (index.length === 0) return false;
  const oldest = index[index.length - 1];
  localStorage.removeItem(SIM_PREFIX + oldest);
  localStorage.removeItem(HIST_PREFIX + oldest);
  index.pop();
  setIndex(index);
  return true;
}

export function saveCacheEntry(
  config: SimConfig,
  stats: ProgressStats,
  meta: { elapsedMs: number; gamesCompleted: number; totalGames: number },
  histories: GameHistory[] | null,
): void {
  const key = computeCacheKey(config);
  const entry: CacheEntry = {
    version: 1,
    key,
    config,
    stats,
    gamesCompleted: meta.gamesCompleted,
    totalGames: meta.totalGames,
    elapsedMs: meta.elapsedMs,
    hasHistories: histories !== null && histories.length > 0,
    savedAt: Date.now(),
  };

  const entryJson = JSON.stringify(entry);

  // Try to save stats entry, evicting oldest on failure
  let saved = false;
  for (let i = 0; i < 4; i++) {
    if (trySetItem(SIM_PREFIX + key, entryJson)) {
      saved = true;
      break;
    }
    if (!evictOldest()) break;
  }
  if (!saved) return;

  // Try to save histories separately
  if (histories && histories.length > 0) {
    const histJson = JSON.stringify(histories);
    let histSaved = false;
    for (let i = 0; i < 4; i++) {
      if (trySetItem(HIST_PREFIX + key, histJson)) {
        histSaved = true;
        break;
      }
      if (!evictOldest()) break;
    }
    if (!histSaved) {
      // Save stats-only version
      entry.hasHistories = false;
      trySetItem(SIM_PREFIX + key, JSON.stringify(entry));
    }
  }

  touchIndex(key);
}

export function listCacheEntries(): CacheEntry[] {
  const index = getIndex();
  const entries: CacheEntry[] = [];
  for (const key of index) {
    try {
      const raw = localStorage.getItem(SIM_PREFIX + key);
      if (raw) {
        const entry: CacheEntry = JSON.parse(raw);
        if (entry.version === 1) entries.push(entry);
      }
    } catch {
      // skip corrupt entries
    }
  }
  return entries;
}

export function deleteCacheEntry(key: string): void {
  localStorage.removeItem(SIM_PREFIX + key);
  localStorage.removeItem(HIST_PREFIX + key);
  const index = getIndex().filter(k => k !== key);
  setIndex(index);
}

export function clearCache(): void {
  const index = getIndex();
  for (const key of index) {
    localStorage.removeItem(SIM_PREFIX + key);
    localStorage.removeItem(HIST_PREFIX + key);
  }
  setIndex([]);
}

export function exportCacheEntry(key: string): string | null {
  try {
    const raw = localStorage.getItem(SIM_PREFIX + key);
    if (!raw) return null;
    const entry: CacheEntry = JSON.parse(raw);

    const histRaw = localStorage.getItem(HIST_PREFIX + key);
    const histories: GameHistory[] | null = histRaw ? JSON.parse(histRaw) : null;

    const exportObj: CacheExportFile = {
      format: 'skyjo-sim-cache',
      version: 1,
      config: entry.config,
      stats: entry.stats,
      gamesCompleted: entry.gamesCompleted,
      totalGames: entry.totalGames,
      elapsedMs: entry.elapsedMs,
      histories,
      exportedAt: Date.now(),
    };
    return JSON.stringify(exportObj);
  } catch {
    return null;
  }
}

export function importCacheEntry(json: string): string | null {
  try {
    const obj: CacheExportFile = JSON.parse(json);
    if (obj.format !== 'skyjo-sim-cache' || obj.version !== 1) return null;
    if (!obj.config || !obj.stats) return null;

    saveCacheEntry(obj.config, obj.stats, {
      elapsedMs: obj.elapsedMs,
      gamesCompleted: obj.gamesCompleted,
      totalGames: obj.totalGames,
    }, obj.histories);

    return computeCacheKey(obj.config);
  } catch {
    return null;
  }
}

export function getCacheSizeEstimate(): { used: number; entries: number } {
  const index = getIndex();
  let used = 0;
  for (const key of index) {
    const sim = localStorage.getItem(SIM_PREFIX + key);
    const hist = localStorage.getItem(HIST_PREFIX + key);
    if (sim) used += sim.length * 2;
    if (hist) used += hist.length * 2;
  }
  return { used, entries: index.length };
}
