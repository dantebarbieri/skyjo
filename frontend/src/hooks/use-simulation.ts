import { useState, useRef, useCallback } from 'react';
import type { GameHistory, ProgressStats, SimConfig, WorkerResponse, CacheExportFile, CacheEntry } from '../types';
import { saveCacheEntry, getCacheEntry, getCacheHistories } from '../cache';

export type SimStatus = 'idle' | 'running' | 'paused' | 'complete' | 'cached';

export interface SimulationState {
  status: SimStatus;
  stats: ProgressStats | null;
  gamesCompleted: number;
  totalGames: number;
  elapsedMs: number;
  histories: GameHistory[] | null;
  realtimeHistory: GameHistory | null;
  error: string | null;
  config: SimConfig | null;
}

export function useSimulation() {
  const [status, setStatus] = useState<SimStatus>('idle');
  const [stats, setStats] = useState<ProgressStats | null>(null);
  const [gamesCompleted, setGamesCompleted] = useState(0);
  const [totalGames, setTotalGames] = useState(0);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [histories, setHistories] = useState<GameHistory[] | null>(null);
  const [realtimeHistory, setRealtimeHistory] = useState<GameHistory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [config, setConfig] = useState<SimConfig | null>(null);

  const workerRef = useRef<Worker | null>(null);
  const configRef = useRef<SimConfig | null>(null);

  const cleanup = useCallback(() => {
    if (workerRef.current) {
      workerRef.current.terminate();
      workerRef.current = null;
    }
  }, []);

  const start = useCallback((cfg: SimConfig) => {
    cleanup();
    setError(null);
    setHistories(null);
    setRealtimeHistory(null);
    setStats(null);
    setGamesCompleted(0);
    setTotalGames(cfg.num_games);
    setElapsedMs(0);
    setConfig(cfg);
    setStatus('running');
    configRef.current = cfg;

    const worker = new Worker(new URL('../worker.ts', import.meta.url), { type: 'module' });
    workerRef.current = worker;

    worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
      const msg = e.data;
      switch (msg.type) {
        case 'ready': {
          const geneticStrategy = cfg.strategies.find(s => s.startsWith('Genetic'));
          if (geneticStrategy) {
            const savedName = geneticStrategy.startsWith('Genetic:') ? geneticStrategy.slice(8) : null;
            const url = savedName
              ? `/api/genetic/saved/${encodeURIComponent(savedName)}/model`
              : '/api/genetic/model';
            fetch(url)
              .then(res => {
                if (!res.ok) throw new Error('Server unavailable');
                return res.json();
              })
              .then(modelData => {
                worker.postMessage({
                  type: 'setGeneticGenome',
                  genome: modelData.best_genome,
                  gamesTrained: modelData.total_games_trained,
                });
                worker.postMessage({ type: 'start', config: cfg });
              })
              .catch(() => {
                setError('Could not download genetic model. Make sure the game server is running.');
                setStatus('idle');
              });
          } else {
            worker.postMessage({ type: 'start', config: cfg });
          }
          break;
        }
        case 'progress':
          setStats(msg.stats);
          setGamesCompleted(msg.gamesCompleted);
          setTotalGames(msg.totalGames);
          setElapsedMs(msg.elapsedMs);
          break;
        case 'realtimeGame':
          setRealtimeHistory(msg.history);
          break;
        case 'complete':
          setStats(msg.stats);
          setGamesCompleted(msg.gamesCompleted);
          setTotalGames(msg.totalGames);
          setElapsedMs(msg.elapsedMs);
          if (msg.histories) {
            setHistories(msg.histories);
          }
          setStatus('complete');
          // Auto-save to cache
          if (msg.gamesCompleted === msg.totalGames && configRef.current) {
            saveCacheEntry(configRef.current, msg.stats, {
              elapsedMs: msg.elapsedMs,
              gamesCompleted: msg.gamesCompleted,
              totalGames: msg.totalGames,
            }, msg.histories);
          }
          worker.terminate();
          workerRef.current = null;
          break;
        case 'error':
          setError(msg.message);
          setStatus('idle');
          worker.terminate();
          workerRef.current = null;
          break;
      }
    };

    worker.onerror = (e) => {
      setError(`Worker error: ${e.message}`);
      setStatus('idle');
      cleanup();
    };
  }, [cleanup]);

  const pause = useCallback(() => {
    workerRef.current?.postMessage({ type: 'pause' });
    setStatus('paused');
  }, []);

  const resume = useCallback(() => {
    workerRef.current?.postMessage({ type: 'resume' });
    setStatus('running');
  }, []);

  const stop = useCallback(() => {
    workerRef.current?.postMessage({ type: 'stop' });
  }, []);

  const requestRealtimeGame = useCallback(() => {
    workerRef.current?.postMessage({ type: 'requestRealtimeGame' });
  }, []);

  const loadFromCache = useCallback((cached: CacheEntry) => {
    cleanup();
    setError(null);
    setConfig(cached.config);
    setStats(cached.stats);
    setGamesCompleted(cached.gamesCompleted);
    setTotalGames(cached.totalGames);
    setElapsedMs(cached.elapsedMs);
    setRealtimeHistory(null);
    setStatus('cached');
    configRef.current = cached.config;

    if (cached.hasHistories) {
      const h = getCacheHistories(cached.config);
      setHistories(h);
    } else {
      setHistories(null);
    }
  }, [cleanup]);

  const loadCacheResult = useCallback((
    s: ProgressStats,
    cfg: SimConfig,
    h: GameHistory[] | null,
    meta: { gamesCompleted: number; totalGames: number; elapsedMs: number }
  ) => {
    cleanup();
    setError(null);
    setConfig(cfg);
    setStats(s);
    setGamesCompleted(meta.gamesCompleted);
    setTotalGames(meta.totalGames);
    setElapsedMs(meta.elapsedMs);
    setHistories(h);
    setRealtimeHistory(null);
    setStatus('cached');
    configRef.current = cfg;
  }, [cleanup]);

  const exportResult = useCallback(() => {
    if (!config || !stats) return;
    const exportObj: CacheExportFile = {
      format: 'skyjo-sim-cache',
      version: 1,
      config,
      stats,
      gamesCompleted,
      totalGames,
      elapsedMs,
      histories: histories && histories.length > 0 ? histories : null,
      exportedAt: Date.now(),
    };
    const json = JSON.stringify(exportObj);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `skyjo-sim-${config.seed}-${config.num_games}g.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [config, stats, gamesCompleted, totalGames, elapsedMs, histories]);

  const load = useCallback((cfg: SimConfig): CacheEntry | null => {
    return getCacheEntry(cfg);
  }, []);

  return {
    status,
    stats,
    gamesCompleted,
    totalGames,
    elapsedMs,
    histories,
    realtimeHistory,
    error,
    config,
    start,
    pause,
    resume,
    stop,
    requestRealtimeGame,
    loadFromCache,
    loadCacheResult,
    exportResult,
    load,
  };
}
