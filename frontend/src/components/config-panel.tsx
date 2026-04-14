import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import RulesInfo from './rules-info';
import type { SimConfig } from '../types';
import type { SimStatus } from '@/hooks/use-simulation';

const DEFAULTS: { numGames: number; seed: number; maxTurns: number; playerCount: number } = {
  numGames: 100,
  seed: 42,
  maxTurns: 10000,
  playerCount: 4,
};

interface ConfigPanelProps {
  strategies: string[];
  rules: string[];
  onStart: (config: SimConfig) => void;
  simRunning: boolean;
  onPause: () => void;
  onResume: () => void;
  onStop: () => void;
  simStatus: SimStatus;
}

export default function ConfigPanel({
  strategies,
  rules,
  onStart,
  simRunning,
  onPause,
  onResume,
  onStop,
  simStatus,
}: ConfigPanelProps) {
  const [numGames, setNumGames] = useState(DEFAULTS.numGames);
  const [seed, setSeed] = useState(DEFAULTS.seed);
  const [maxTurns, setMaxTurns] = useState(DEFAULTS.maxTurns);
  const [selectedRules, setSelectedRules] = useState(rules[0] ?? '');
  const [playerCount, setPlayerCount] = useState(DEFAULTS.playerCount);
  const [playerStrategies, setPlayerStrategies] = useState<string[]>([]);
  const [bulkStrategy, setBulkStrategy] = useState(strategies[0] ?? '');
  const [savedGens, setSavedGens] = useState<{ name: string; generation: number; lineage_hash: string; best_fitness: number }[]>([]);

  // Fetch saved genetic generations
  useEffect(() => {
    if (!strategies.includes('Genetic')) return;
    fetch('/api/genetic/saved')
      .then(res => res.ok ? res.json() : [])
      .then(data => setSavedGens(data.map((sg: { name: string; generation: number; lineage_hash: string; best_fitness: number }) => ({
        name: sg.name, generation: sg.generation, lineage_hash: sg.lineage_hash, best_fitness: sg.best_fitness,
      }))))
      .catch(() => {});
  }, [strategies]);

  useEffect(() => {
    const strats: string[] = [];
    for (let i = 0; i < playerCount; i++) {
      strats.push(strategies.length > 1 && i % 2 === 1 ? strategies[1] : strategies[0]);
    }
    setPlayerStrategies(strats);
    setBulkStrategy(strategies[0] ?? '');
  }, [playerCount, strategies]);

  const handleStart = (withHistories: boolean) => {
    onStart({
      num_games: numGames,
      seed,
      strategies: playerStrategies,
      rules: selectedRules,
      withHistories,
      realtimeVisualization: true,
      maxTurnsPerRound: maxTurns,
    });
  };

  const updateStrategy = (index: number, value: string) => {
    setPlayerStrategies((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
  };

  const handleReset = () => {
    setNumGames(DEFAULTS.numGames);
    setSeed(DEFAULTS.seed);
    setMaxTurns(DEFAULTS.maxTurns);
    setSelectedRules(rules[0] ?? '');
    setPlayerCount(DEFAULTS.playerCount);
    // playerStrategies resets via the useEffect watching playerCount
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Configuration</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Number of games</label>
            <Input
              type="number"
              value={numGames}
              onChange={(e) => setNumGames(parseInt(e.target.value) || 1)}
              min={1}
              max={1000000}
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Seed</label>
            <div className="flex gap-2">
              <Input
                type="number"
                value={seed}
                onChange={(e) => setSeed(parseInt(e.target.value) || 0)}
                min={0}
                className="flex-1"
              />
              <Button
                variant="outline"
                size="sm"
                onClick={() => setSeed(Math.floor(Math.random() * 1_000_000))}
              >
                Random
              </Button>
            </div>
          </div>
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Max turns/round</label>
            <Input
              type="number"
              value={maxTurns}
              onChange={(e) => setMaxTurns(parseInt(e.target.value) || 100)}
              min={100}
              max={100000}
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Rules</label>
            <Select value={selectedRules} onValueChange={setSelectedRules}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {rules.map((r) => (
                  <SelectItem key={r} value={r}>{r}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Number of players</label>
            <Select value={String(playerCount)} onValueChange={(v) => setPlayerCount(parseInt(v))}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[2, 3, 4, 5, 6, 7, 8].map((n) => (
                  <SelectItem key={n} value={String(n)}>{n}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <Collapsible>
          <CollapsibleTrigger className="text-sm text-muted-foreground hover:text-foreground transition-colors">
            Rules: {selectedRules} ...
          </CollapsibleTrigger>
          <CollapsibleContent className="mt-2">
            <RulesInfo rulesName={selectedRules} />
          </CollapsibleContent>
        </Collapsible>

        <fieldset className="border rounded-lg p-3">
          <legend className="text-sm font-medium px-1">Player Strategies</legend>
          <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-2 mb-2">
            <Select value={bulkStrategy} onValueChange={setBulkStrategy}>
              <SelectTrigger className="w-full sm:w-48">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {strategies.map((s) => (
                  <SelectItem key={s} value={s}>{s}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPlayerStrategies(Array(playerCount).fill(bulkStrategy))}
            >
              Apply to All
            </Button>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {playerStrategies.map((strat, i) => (
              <div key={i} className="flex items-center gap-2">
                <label className="text-sm text-muted-foreground w-16 sm:w-20 shrink-0">
                  Player {i + 1}:
                </label>
                <Select value={strat} onValueChange={(v) => updateStrategy(i, v)}>
                  <SelectTrigger className="flex-1">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {strategies.map((s) => (
                      <SelectItem key={s} value={s}>{s}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {/* Generation picker for Genetic */}
                {strat.startsWith('Genetic') && (
                  <Select
                    value={strat === 'Genetic' ? '__latest__' : strat.slice(8)}
                    onValueChange={(v) => updateStrategy(i, v === '__latest__' ? 'Genetic' : `Genetic:${v}`)}
                  >
                    <SelectTrigger className="w-32 shrink-0 text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__latest__">Latest</SelectItem>
                      {(() => {
                        const hasMultipleLineages = new Set(savedGens.map(g => g.lineage_hash)).size > 1;
                        return savedGens.map((sg) => (
                          <SelectItem key={sg.name} value={sg.name}>
                            <span>{sg.name}</span>
                            {hasMultipleLineages && sg.lineage_hash && (
                              <span className="ml-1 font-mono text-muted-foreground">{sg.lineage_hash.slice(0, 4)}</span>
                            )}
                            <span className="ml-1 text-muted-foreground">({sg.best_fitness.toFixed(0)})</span>
                          </SelectItem>
                        ));
                      })()}
                    </SelectContent>
                  </Select>
                )}
                <Link
                  to={`/rules/strategies/${strat.startsWith('Genetic:') ? 'Genetic' : strat}`}
                  className="text-muted-foreground hover:text-primary transition-colors shrink-0"
                  title={`View ${strat.startsWith('Genetic:') ? 'Genetic' : strat} strategy guide`}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
                </Link>
              </div>
            ))}
          </div>
        </fieldset>

        <div className="flex gap-2 flex-wrap">
          <Button onClick={() => handleStart(false)} disabled={simRunning}>
            Run Simulation
          </Button>
          <Button onClick={() => handleStart(true)} disabled={simRunning} variant="secondary">
            Run & Save Histories
          </Button>
          <Button variant="ghost" size="sm" onClick={handleReset} disabled={simRunning}>
            Reset
          </Button>
          {simRunning && (
            <>
              <Button
                onClick={simStatus === 'paused' ? onResume : onPause}
                variant="outline"
              >
                {simStatus === 'paused' ? 'Resume' : 'Pause'}
              </Button>
              <Button onClick={onStop} variant="destructive">
                Stop
              </Button>
            </>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
