import { useState, useCallback, useRef, useEffect } from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { useWasmContext } from '@/contexts/wasm-context';
import { useSimulation } from '@/hooks/use-simulation';
import { useCache } from '@/hooks/use-cache';
import ConfigPanel from '@/components/config-panel';
import ProgressSection from '@/components/progress-section';
import StatsTable from '@/components/stats-table';
import GameList from '@/components/game-list';
import ReplaySection from '@/components/replay-section';
import RealtimeSection from '@/components/realtime-section';
import CachePanel from '@/components/cache-panel';
import ScoringSheet from '@/components/scoring-sheet';
import type { GameHistory, SimConfig, ProgressStats } from '@/types';

export default function SimulatorRoute() {
  const wasm = useWasmContext();
  const sim = useSimulation();
  const cache = useCache();
  const [replayHistory, setReplayHistory] = useState<GameHistory | null>(null);
  const [selectedGameIndex, setSelectedGameIndex] = useState<number | null>(null);
  const progressRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to progress when simulation starts
  useEffect(() => {
    if (sim.status === 'running') {
      progressRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }, [sim.status]);

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

  const handleViewGame = useCallback((history: GameHistory, index: number) => {
    setReplayHistory(history);
    setSelectedGameIndex(index);
    // Auto-scroll to the combined view section after render
    requestAnimationFrame(() => {
      viewRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
  }, []);

  const handleCloseView = useCallback(() => {
    setReplayHistory(null);
    setSelectedGameIndex(null);
  }, []);

  const handleLoadFromCache = useCallback((stats: ProgressStats, config: SimConfig, histories: GameHistory[] | null, meta: { gamesCompleted: number; totalGames: number; elapsedMs: number }) => {
    setReplayHistory(null);
    setSelectedGameIndex(null);
    sim.loadCacheResult(stats, config, histories, meta);
  }, [sim]);

  return (
    <>
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

        <div ref={progressRef}>
          <ProgressSection
            status={sim.status}
            gamesCompleted={sim.gamesCompleted}
            totalGames={sim.totalGames}
            elapsedMs={sim.elapsedMs}
            stats={sim.stats}
            onPause={sim.pause}
            onResume={sim.resume}
            onStop={sim.stop}
          />
        </div>

        {(sim.status === 'running' || sim.status === 'paused') && (
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
            onView={handleViewGame}
            selectedIndex={selectedGameIndex}
          />
        )}

        {(selectedGameIndex !== null || replayHistory) && (
          <div ref={viewRef} className="space-y-6">
            {selectedGameIndex !== null && sim.histories?.[selectedGameIndex] && (
              <Card>
                <CardContent className="pt-6">
                  <ScoringSheet
                    history={sim.histories[selectedGameIndex]}
                    onClose={handleCloseView}
                  />
                </CardContent>
              </Card>
            )}
            {replayHistory && (
              <ReplaySection
                history={replayHistory}
                gameNumber={selectedGameIndex ?? undefined}
                onClose={handleCloseView}
              />
            )}
          </div>
        )}
      </div>
    </>
  );
}
