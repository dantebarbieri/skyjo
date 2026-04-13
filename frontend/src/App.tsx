import { useState, useCallback } from 'react';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useWasm } from '@/hooks/use-wasm';
import { useSimulation } from '@/hooks/use-simulation';
import { useCache } from '@/hooks/use-cache';
import ConfigPanel from '@/components/config-panel';
import ProgressSection from '@/components/progress-section';
import StatsTable from '@/components/stats-table';
import GameList from '@/components/game-list';
import ReplaySection from '@/components/replay-section';
import RealtimeSection from '@/components/realtime-section';
import CachePanel from '@/components/cache-panel';
import type { GameHistory, SimConfig, ProgressStats } from './types';

export default function App() {
  const wasm = useWasm();
  const sim = useSimulation();
  const cache = useCache();
  const [replayHistory, setReplayHistory] = useState<GameHistory | null>(null);
  const [selectedGameIndex, setSelectedGameIndex] = useState<number | null>(null);

  const handleStartSimulation = useCallback((config: SimConfig) => {
    setReplayHistory(null);
    setSelectedGameIndex(null);

    // Check cache first
    const cached = cache.load(config);
    if (cached && (!config.withHistories || cached.hasHistories)) {
      sim.loadFromCache(cached);
      return;
    }

    sim.start(config);
  }, [sim, cache]);

  const handleOpenReplay = useCallback((history: GameHistory, index: number) => {
    setReplayHistory(history);
    setSelectedGameIndex(index);
  }, []);

  const handleCloseReplay = useCallback(() => {
    setReplayHistory(null);
    setSelectedGameIndex(null);
  }, []);

  const handleLoadFromCache = useCallback((stats: ProgressStats, config: SimConfig, histories: GameHistory[] | null, meta: { gamesCompleted: number; totalGames: number; elapsedMs: number }) => {
    setReplayHistory(null);
    setSelectedGameIndex(null);
    sim.loadCacheResult(stats, config, histories, meta);
  }, [sim]);

  if (!wasm.ready) {
    return (
      <div className="min-h-screen bg-background text-foreground flex items-center justify-center">
        {wasm.error ? (
          <div className="text-destructive text-center">
            <p className="text-lg font-semibold">Failed to load WASM module</p>
            <p className="text-sm mt-2">{wasm.error}</p>
          </div>
        ) : (
          <div className="text-muted-foreground animate-pulse text-lg">
            Loading Skyjo Simulator...
          </div>
        )}
      </div>
    );
  }

  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground">
        <div className="mx-auto max-w-7xl px-4 py-6">
          <h1 className="text-3xl font-bold mb-6">Skyjo Simulator</h1>

          <div className="space-y-6">
            <ConfigPanel
              strategies={wasm.strategies}
              rules={wasm.rules}
              onStart={handleStartSimulation}
              simRunning={sim.status === 'running' || sim.status === 'paused'}
              onPause={sim.pause}
              onResume={sim.resume}
              onStop={sim.stop}
              simStatus={sim.status}
            />

            <CachePanel
              onLoad={handleLoadFromCache}
            />

            {sim.error && (
              <div className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive">
                {sim.error}
              </div>
            )}

            <ProgressSection
              status={sim.status}
              gamesCompleted={sim.gamesCompleted}
              totalGames={sim.totalGames}
              elapsedMs={sim.elapsedMs}
              stats={sim.stats}
            />

            {sim.status === 'running' && (
              <RealtimeSection
                history={sim.realtimeHistory}
                strategyNames={sim.config?.strategies ?? []}
                onNeedNextGame={sim.requestRealtimeGame}
              />
            )}

            {sim.stats && (
              <StatsTable
                stats={sim.stats}
                strategyNames={sim.config?.strategies ?? []}
                gamesCompleted={sim.gamesCompleted}
                onExport={sim.config ? () => sim.exportResult() : undefined}
              />
            )}

            {sim.histories && sim.histories.length > 0 && (
              <GameList
                histories={sim.histories}
                onReplay={handleOpenReplay}
                selectedIndex={selectedGameIndex}
              />
            )}

            {replayHistory && (
              <ReplaySection
                history={replayHistory}
                onClose={handleCloseReplay}
              />
            )}
          </div>
        </div>
      </div>
    </TooltipProvider>
  );
}
