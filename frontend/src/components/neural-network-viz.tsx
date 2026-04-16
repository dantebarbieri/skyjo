import { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Input } from '@/components/ui/input';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { useAuth } from '@/contexts/auth-context';
import { apiFetch } from '@/lib/api';
import type { GeneticModelData, GeneticTrainingStatus, SavedGenerationInfo } from '@/types';

const API_BASE = '/api';

async function extractErrorMessage(res: Response): Promise<string> {
  try {
    const body = await res.json();
    return body?.error?.message || body?.message || `Request failed (HTTP ${res.status})`;
  } catch {
    return `Request failed (HTTP ${res.status})`;
  }
}

interface NeuralNetworkVizProps {
  className?: string;
}

export function NeuralNetworkViz({ className }: NeuralNetworkVizProps) {
  const { user, isAuthenticated } = useAuth();
  const canManage = isAuthenticated && user && (user.permission === 'admin' || user.permission === 'moderator');
  const [model, setModel] = useState<GeneticModelData | null>(null);
  const [status, setStatus] = useState<GeneticTrainingStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [savedGenerations, setSavedGenerations] = useState<SavedGenerationInfo[]>([]);
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastGenRef = useRef(0);
  const pollIntervalRef = useRef(500);
  const pollHistoryRef = useRef<boolean[]>([]);
  // Client-side elapsed time interpolation
  const elapsedAnchorRef = useRef<{ serverMs: number; localTs: number } | null>(null);
  const [clientElapsedMs, setClientElapsedMs] = useState(0);
  const elapsedRafRef = useRef<number | null>(null);
  const [trainGenCount, setTrainGenCount] = useState('50');
  const [trainTargetGen, setTrainTargetGen] = useState('');
  const [trainTargetFitness, setTrainTargetFitness] = useState('-30');
  const [trainError, setTrainError] = useState<string | null>(null);
  const [showResetDialog, setShowResetDialog] = useState(false);

  const fetchModel = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/genetic/model`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setModel(await res.json());
      setError(null);
    } catch {
      setError('Could not connect to server. Neural network visualization requires the game server.');
    }
  }, []);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/genetic/status`);
      if (!res.ok) return null;
      const s: GeneticTrainingStatus = await res.json();
      setStatus(s);
      // Seed elapsed time anchor on every status fetch
      if (s.is_training) {
        elapsedAnchorRef.current = { serverMs: s.training_elapsed_ms, localTs: performance.now() };
      }
      return s;
    } catch {
      return null;
    }
  }, []);

  const fetchSaved = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/genetic/saved`);
      if (res.ok) setSavedGenerations(await res.json());
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    fetchModel();
    fetchStatus();
    fetchSaved();
  }, [fetchModel, fetchStatus, fetchSaved]);

  // Adaptive polling: speeds up when data changes, slows down when stale
  const POLL_MIN_MS = 250;
  const POLL_MAX_MS = 5000;
  const POLL_WINDOW = 5;

  const schedulePoll = useCallback(() => {
    stopPolling();
    pollRef.current = setTimeout(async () => {
      const s = await fetchStatus();
      // Freshness: only check generation and last-gen ETA snapshot (not elapsed time)
      const hadNewData = s != null && (
        s.generation !== lastGenRef.current ||
        s.training_last_gen_elapsed_ms !== (statusRef.current?.training_last_gen_elapsed_ms ?? 0)
      );
      if (s) {
        statusRef.current = s;
        // Anchor client-side elapsed time to server value
        elapsedAnchorRef.current = { serverMs: s.training_elapsed_ms, localTs: performance.now() };
      }

      if (s && s.generation !== lastGenRef.current) {
        lastGenRef.current = s.generation;
        await fetchModel();
      }
      if (s && !s.is_training) {
        stopPolling();
        stopElapsedTimer();
        await fetchModel();
        await fetchSaved();
        return;
      }

      // Track recent poll results and adapt interval
      const history = pollHistoryRef.current;
      history.push(hadNewData);
      if (history.length > POLL_WINDOW) history.shift();

      const freshCount = history.filter(Boolean).length;
      let interval = pollIntervalRef.current;
      if (freshCount >= POLL_WINDOW) {
        interval = Math.max(POLL_MIN_MS, interval * 0.7);
      } else if (freshCount === 0) {
        interval = Math.min(POLL_MAX_MS, interval * 1.5);
      }
      pollIntervalRef.current = interval;

      schedulePoll();
    }, pollIntervalRef.current);
  }, [fetchStatus, fetchModel, fetchSaved]);

  // Keep a ref to the latest status for comparison without triggering re-renders
  const statusRef = useRef<GeneticTrainingStatus | null>(null);

  // Client-side elapsed time: interpolate from last server anchor using requestAnimationFrame
  function startElapsedTimer() {
    stopElapsedTimer();
    const tick = () => {
      const anchor = elapsedAnchorRef.current;
      if (anchor) {
        const delta = performance.now() - anchor.localTs;
        setClientElapsedMs(anchor.serverMs + delta);
      }
      elapsedRafRef.current = requestAnimationFrame(tick);
    };
    elapsedRafRef.current = requestAnimationFrame(tick);
  }

  function stopElapsedTimer() {
    if (elapsedRafRef.current != null) {
      cancelAnimationFrame(elapsedRafRef.current);
      elapsedRafRef.current = null;
    }
  }

  const startPolling = useCallback(() => {
    pollIntervalRef.current = POLL_MIN_MS;
    pollHistoryRef.current = [];
    schedulePoll();
    startElapsedTimer();
  }, [schedulePoll]);

  function stopPolling() {
    if (pollRef.current) { clearTimeout(pollRef.current); pollRef.current = null; }
  }

  useEffect(() => () => { stopPolling(); stopElapsedTimer(); }, []);

  async function startTraining(request: Record<string, unknown>) {
    setTrainError(null);
    try {
      const res = await apiFetch(`${API_BASE}/genetic/train`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });
      if (!res.ok) {
        setTrainError(await extractErrorMessage(res));
        return;
      }
      const s: GeneticTrainingStatus = await res.json();
      setStatus(s);
      lastGenRef.current = s.generation;
      startPolling();
    } catch {
      setTrainError('Failed to connect to server');
    }
  }

  function handleTrainForGenerations() {
    const n = parseInt(trainGenCount, 10);
    if (isNaN(n) || n <= 0) { setTrainError('Enter a positive number'); return; }
    startTraining({ mode: 'generations', generations: n });
  }

  function handleTrainUntilGeneration() {
    const target = parseInt(trainTargetGen, 10);
    if (isNaN(target) || target <= 0) { setTrainError('Enter a positive target'); return; }
    const current = status?.generation ?? model?.generation ?? 0;
    if (target <= current) { setTrainError(`Target must be > current generation (${current})`); return; }
    startTraining({ mode: 'until_generation', target_generation: target });
  }

  function handleTrainUntilFitness() {
    const f = parseFloat(trainTargetFitness);
    if (isNaN(f)) { setTrainError('Enter a valid fitness value'); return; }
    startTraining({ mode: 'until_fitness', target_fitness: f });
  }

  async function handleCancel() {
    try {
      const res = await apiFetch(`${API_BASE}/genetic/stop`, { method: 'POST' });
      if (res.ok) {
        const s: GeneticTrainingStatus = await res.json();
        setStatus(s);
      }
    } catch {
      // ignore
    }
  }

  async function handleReset() {
    setShowResetDialog(false);
    try {
      const res = await apiFetch(`${API_BASE}/genetic/reset`, { method: 'POST' });
      if (res.ok) {
        const s: GeneticTrainingStatus = await res.json();
        setStatus(s);
        await fetchModel();
        await fetchSaved();
      } else {
        setTrainError(await extractErrorMessage(res));
      }
    } catch {
      setTrainError('Failed to connect to server');
    }
  }

  async function handleLoadSaved(name: string) {
    if (name === '__current__') return;
    setTrainError(null);
    try {
      const res = await apiFetch(`${API_BASE}/genetic/load`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      });
      if (res.ok) {
        const s: GeneticTrainingStatus = await res.json();
        setStatus(s);
        await fetchModel();
      } else {
        setTrainError(await extractErrorMessage(res));
      }
    } catch {
      setTrainError('Failed to connect to server');
    }
  }

  // Resume polling if we load the page and training is already in progress
  useEffect(() => {
    if (status?.is_training && !pollRef.current) {
      lastGenRef.current = status.generation;
      startPolling();
    }
  }, [status?.is_training, startPolling]);

  // Training progress calculations (all timing from server)
  const isTraining = status?.is_training ?? false;
  const [hasSeenTraining, setHasSeenTraining] = useState(false);
  const [wasCancelled, setWasCancelled] = useState(false);
  const wasTrainingRef = useRef(false);
  // Snapshot last training stats so we can display them after completion
  const lastTrainingStatsRef = useRef<{
    elapsedSec: number;
    gensPerSec: number;
    mutationRate: number;
    mutationSigma: number;
    gensDone: number;
    targetGen: number;
    targetFitness: number;
    mode: string;
    finalFitness: number;
  } | null>(null);
  useEffect(() => {
    if (isTraining) {
      setHasSeenTraining(true);
      setWasCancelled(false);
      wasTrainingRef.current = true;
    } else if (wasTrainingRef.current) {
      // Training just stopped — determine if cancelled or completed
      wasTrainingRef.current = false;
      if (status) {
        const reachedGenTarget = status.generation >= status.training_target_generation;
        const reachedFitnessTarget = status.training_mode === 'until_fitness'
          && status.best_fitness >= status.training_target_fitness;
        if (!reachedGenTarget && !reachedFitnessTarget) {
          setWasCancelled(true);
        }
      }
    }
  }, [isTraining, status]);
  const trainingMode = status?.training_mode ?? 'generations';
  const isFitnessMode = trainingMode === 'until_fitness';
  const gensDone = isTraining ? (status!.generation - status!.training_start_generation) : 0;
  const gensTotal = isTraining ? (status!.training_target_generation - status!.training_start_generation) : 0;
  const gensRemaining = gensTotal - gensDone;
  const elapsedSec = isTraining ? (clientElapsedMs / 1000) : 0;
  // Use the snapshot at last gen completion for stable rate/ETA (avoids drift between polls)
  const stableElapsedSec = isTraining ? (status!.training_last_gen_elapsed_ms / 1000) : 0;
  const gensPerSec = stableElapsedSec > 0 && gensDone > 0 ? gensDone / stableElapsedSec : 0;
  // Generation-based ETA (works for all modes — fitness mode has a safety cap)
  const genEtaSec = gensDone > 0 ? gensRemaining * (stableElapsedSec / gensDone) : 0;
  // Fitness-based ETA: extrapolate from improvement rate (approximate)
  const fitnessEtaSec = (() => {
    if (!isFitnessMode || !isTraining || stableElapsedSec <= 0) return 0;
    const improvement = status!.best_fitness - status!.training_start_fitness;
    if (improvement <= 0) return 0; // no improvement yet
    const remaining = status!.training_target_fitness - status!.best_fitness;
    if (remaining <= 0) return 0; // already reached
    return remaining * (stableElapsedSec / improvement);
  })();
  // For fitness mode: show fitness ETA if available, otherwise fall back to gen-based ETA against safety cap
  const etaSec = isFitnessMode
    ? (fitnessEtaSec > 0 ? Math.min(fitnessEtaSec, genEtaSec) : genEtaSec)
    : genEtaSec;

  // Snapshot training stats while active so we can show them after completion
  if (isTraining && status) {
    lastTrainingStatsRef.current = {
      elapsedSec,
      gensPerSec,
      mutationRate: status.current_mutation_rate,
      mutationSigma: status.current_mutation_sigma,
      gensDone,
      targetGen: status.training_target_generation,
      targetFitness: status.training_target_fitness,
      mode: status.training_mode,
      finalFitness: status.best_fitness,
    };
  }
  const completedStats = lastTrainingStatsRef.current;

  function formatTime(sec: number): string {
    if (sec < 60) return `${Math.round(sec)}s`;
    if (sec < 3600) {
      const m = Math.floor(sec / 60);
      const s = Math.round(sec % 60);
      return `${m}m ${s}s`;
    }
    if (sec < 86400) {
      const h = Math.floor(sec / 3600);
      const m = Math.round((sec % 3600) / 60);
      return `${h}h ${m}m`;
    }
    if (sec < 604800) {
      const d = Math.floor(sec / 86400);
      const h = Math.round((sec % 86400) / 3600);
      return `${d}d ${h}h`;
    }
    const w = Math.floor(sec / 604800);
    const d = Math.round((sec % 604800) / 86400);
    return `${w}w ${d}d`;
  }


  if (error) {
    return (
      <Card className={className}>
        <CardContent className="py-8 text-center text-muted-foreground">
          <p className="text-sm">{error}</p>
        </CardContent>
      </Card>
    );
  }

  if (!model) {
    return (
      <Card className={className}>
        <CardContent className="py-8 text-center text-muted-foreground">
          <p className="text-sm">Loading neural network...</p>
        </CardContent>
      </Card>
    );
  }

  const lineageHash = status?.lineage_hash || model.lineage_hash;

  return (
    <div className={`space-y-4 ${className ?? ''}`}>
      {/* 1. Training controls — only shown to moderator+ */}
      {canManage && (
      <Card>
        <CardContent className="py-3 px-4 space-y-3">
          {/* Lineage selector */}
          {savedGenerations.length > 0 && (
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-sm text-muted-foreground shrink-0">Model:</span>
              <Select
                value="__current__"
                onValueChange={handleLoadSaved}
                disabled={isTraining}
              >
                <SelectTrigger className="w-56 text-xs">
                  <SelectValue>
                    Gen {model.generation}{lineageHash ? ` — Lineage ${lineageHash.slice(0, 8)}` : ''}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__current__">
                    <span>Current: Gen {model.generation}</span>
                    {lineageHash && <span className="ml-1 font-mono text-muted-foreground">{lineageHash.slice(0, 8)}</span>}
                  </SelectItem>
                  {savedGenerations.map((sg) => (
                    <SelectItem key={sg.name} value={sg.name}>
                      <span>{sg.name}</span>
                      {sg.lineage_hash && <span className="ml-1 font-mono text-muted-foreground">{sg.lineage_hash.slice(0, 8)}</span>}
                      <span className="ml-1 text-muted-foreground">({sg.best_fitness.toFixed(0)})</span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {/* Training mode tabs */}
          <Tabs defaultValue="for-generations" className="w-full">
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="for-generations" disabled={isTraining}>Train for X gen</TabsTrigger>
              <TabsTrigger value="until-generation" disabled={isTraining}>Until gen X</TabsTrigger>
              <TabsTrigger value="until-fitness" disabled={isTraining}>Until fitness X</TabsTrigger>
            </TabsList>

            <TabsContent value="for-generations" className="flex items-center gap-2 mt-2 flex-wrap">
              <Input
                type="number" min={1} value={trainGenCount}
                onChange={e => setTrainGenCount(e.target.value)}
                className="w-28" placeholder="50"
                disabled={isTraining}
              />
              <Button size="sm" onClick={handleTrainForGenerations} disabled={isTraining}>
                Train
              </Button>
              <p className="text-xs text-muted-foreground w-full">
                Each generation takes ~20–30s. 50 gens ≈ 15–25 min.
              </p>
            </TabsContent>

            <TabsContent value="until-generation" className="flex items-center gap-2 mt-2 flex-wrap">
              <span className="text-sm text-muted-foreground shrink-0">Target:</span>
              <Input
                type="number" min={1} value={trainTargetGen}
                onChange={e => setTrainTargetGen(e.target.value)}
                className="w-28" placeholder={String((status?.generation ?? model?.generation ?? 0) + 50)}
                disabled={isTraining}
              />
              <Button size="sm" onClick={handleTrainUntilGeneration} disabled={isTraining}>
                Train
              </Button>
              <span className="text-xs text-muted-foreground shrink-0">
                Current: {status?.generation ?? model?.generation ?? 0}
              </span>
              <p className="text-xs text-muted-foreground w-full">
                Each generation takes ~20–30s. Target should be current + desired training count.
              </p>
            </TabsContent>

            <TabsContent value="until-fitness" className="flex items-center gap-2 mt-2 flex-wrap">
              <span className="text-sm text-muted-foreground shrink-0">Target:</span>
              <Input
                type="number" step="0.1" value={trainTargetFitness}
                onChange={e => setTrainTargetFitness(e.target.value)}
                className="w-28" placeholder="-30"
                disabled={isTraining}
              />
              <Button size="sm" onClick={handleTrainUntilFitness} disabled={isTraining}>
                Train
              </Button>
              <span className="text-xs text-muted-foreground shrink-0">(50k gen cap)</span>
              <p className="text-xs text-muted-foreground w-full">
                Random starts ≈ −200. After training: −80 is decent, −50 is good, −30 is strong. Less negative = better.
              </p>
            </TabsContent>
          </Tabs>
          {trainError && <p className="text-sm text-destructive">{trainError}</p>}
          <div className="flex gap-2 flex-wrap items-center">
            <a
              href="/rules/strategies/Genetic/manage"
              className="inline-flex items-center gap-1 text-sm font-medium text-primary hover:underline py-1"
            >
              Manage Generations ({savedGenerations.length} saved)
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
            </a>
            {!isTraining && (
              <>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => { fetchModel(); fetchStatus(); fetchSaved(); }}
                >
                  Refresh
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="text-destructive hover:text-destructive"
                  onClick={() => setShowResetDialog(true)}
                >
                  New Lineage
                </Button>
              </>
            )}
          </div>
        </CardContent>
      </Card>
      )}

      {/* 2. Training progress — shown to all users once training is observed */}
      {hasSeenTraining && status && (
        <Card>
          <CardContent className="py-3 px-4 space-y-2">
            {isTraining ? (
              <>
                <div className="flex items-center justify-between text-sm">
                  {isFitnessMode ? (
                    <>
                      <span className="font-medium">
                        Training: Gen {status.generation} | Fitness {status.best_fitness.toFixed(1)} / {status.training_target_fitness.toFixed(1)}
                      </span>
                      <span className="text-muted-foreground text-xs">
                        {gensDone} generations (max {gensTotal})
                      </span>
                    </>
                  ) : (
                    <>
                      <span className="font-medium">
                        Training: Gen {status.generation} / {status.training_target_generation}
                      </span>
                      <span className="text-muted-foreground text-xs">
                        {gensDone} of {gensTotal} generations
                      </span>
                    </>
                  )}
                </div>
                <div className="h-2 bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-primary rounded-full transition-all duration-300"
                    style={{ width: gensTotal > 0 ? `${(gensDone / gensTotal) * 100}%` : '0%' }}
                  />
                </div>
                <div className="flex justify-between items-center text-xs text-muted-foreground">
                  <span>Elapsed: {formatTime(elapsedSec)}</span>
                  {gensPerSec > 0 && <span>{gensPerSec.toFixed(2)} gen/s</span>}
                  {status.current_mutation_rate > 0 && (
                    <span title="Adaptive mutation rate / sigma">
                      μ: {(status.current_mutation_rate * 100).toFixed(1)}% σ: {status.current_mutation_sigma.toFixed(2)}
                    </span>
                  )}
                  {etaSec > 0 && (
                    <span title={isFitnessMode ? (fitnessEtaSec > 0 ? 'Based on fitness improvement rate' : 'Based on generation safety cap') : undefined}>
                      {isFitnessMode ? '~' : ''}ETA: {formatTime(etaSec)}
                    </span>
                  )}
                  {canManage && (
                    <Button size="sm" variant="ghost" className="h-6 px-2 text-xs text-destructive hover:text-destructive" onClick={handleCancel}>
                      Cancel
                    </Button>
                  )}
                </div>
              </>
            ) : (
              <>
                <div className="flex items-center justify-between text-sm">
                  {wasCancelled ? (
                    <span className="font-medium text-amber-600 dark:text-amber-400">
                      Training cancelled — Gen {status.generation}
                    </span>
                  ) : (
                    <span className="font-medium text-green-600 dark:text-green-400">
                      Training complete — Gen {status.generation}
                    </span>
                  )}
                  <span className="text-muted-foreground text-xs">
                    Fitness: {status.best_fitness !== 0 ? status.best_fitness.toFixed(1) : 'N/A'}
                  </span>
                </div>
                <div className="h-2 bg-muted rounded-full overflow-hidden">
                  <div className={`h-full rounded-full ${wasCancelled ? 'bg-amber-500' : 'bg-green-500'}`}
                    style={{ width: wasCancelled && completedStats
                      ? `${Math.min(100, completedStats.gensDone / Math.max(1, completedStats.targetGen - (status.generation - completedStats.gensDone)) * 100)}%`
                      : '100%' }}
                  />
                </div>
                {completedStats && (
                  <div className="flex justify-between items-center text-xs text-muted-foreground">
                    <span>Elapsed: {formatTime(completedStats.elapsedSec)}</span>
                    {completedStats.gensPerSec > 0 && <span>{completedStats.gensPerSec.toFixed(2)} gen/s</span>}
                    {completedStats.mutationRate > 0 && (
                      <span>μ: {(completedStats.mutationRate * 100).toFixed(1)}% σ: {completedStats.mutationSigma.toFixed(2)}</span>
                    )}
                    <span>{completedStats.gensDone} generations</span>
                  </div>
                )}
              </>
            )}
          </CardContent>
        </Card>
      )}

      {/* 3. Model stats badges */}
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="outline" className="text-xs">
          Generation: {model.generation}
        </Badge>
        <Badge variant="outline" className="text-xs">
          Games Trained: {model.total_games_trained.toLocaleString()}
        </Badge>
        <Badge variant="outline" className="text-xs">
          Fitness: {status && status.best_fitness !== 0 ? status.best_fitness.toFixed(1) : 'N/A'}
        </Badge>
        {lineageHash && (
          <Badge variant="outline" className="text-xs">
            Lineage: <span className="font-mono">{lineageHash}</span>
          </Badge>
        )}
        <Badge variant="outline" className="text-xs">
          Architecture: {model.input_size} → {model.hidden1_size || model.hidden_size} → {model.hidden2_size || '?'} → {model.output_size}
          {model.architecture_version ? ` (v${model.architecture_version})` : ''}
        </Badge>
      </div>

      {/* 4. NN Diagram */}
      <Card>
        <CardHeader className="pb-2 pt-3 px-4">
          <CardTitle className="text-sm font-semibold">Network Architecture</CardTitle>
        </CardHeader>
        <CardContent className="px-4 pb-4">
          <NetworkDiagram model={model} />
        </CardContent>
      </Card>

      {/* 5. Glossary */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-x-6 gap-y-1 text-xs text-muted-foreground">
        <p><strong className="text-foreground">Generation</strong> — one cycle of evolution. Each generation, 300 neural networks play games against varied opponents (2-6 players), and the best are selected, crossed over (SBX), and mutated to produce the next generation.</p>
        <p><strong className="text-foreground">Fitness</strong> — how well the best network performs. Calculated as the negative average score across training games. Since lower Skyjo scores are better, fitness values are typically negative. <em>Less negative (closer to 0) = better performance.</em></p>
        <p><strong className="text-foreground">Inputs ({model.input_size})</strong> — what the network sees: board state, discard pile, deck size, scores, column match potential, drawn card, opponent hidden counts, opponent near-done signals, card counting distribution (remaining copies of each value), and round progress.</p>
        <p><strong className="text-foreground">Hidden ({model.hidden1_size || model.hidden_size} + {model.hidden2_size || '?'})</strong> — two layers of internal neurons with ReLU activation that learn patterns from the inputs.</p>
        <p><strong className="text-foreground">Outputs ({model.output_size})</strong> — decisions the network makes: which cards to flip, whether to draw from deck or discard, whether to keep or swap, and where to place cards.</p>
        <p><strong className="text-foreground">Lineage</strong> — a unique identifier for each independent training run. When you reset the model, a new lineage begins. Saved generations retain their lineage so you can compare models from different training runs.</p>
        <p><strong className="text-foreground">ReLU</strong> — Rectified Linear Unit. An activation function: outputs the input if positive, 0 otherwise. Formula: max(0, x). Makes the network capable of learning non-linear patterns.</p>
        <p><strong className="text-foreground">Weight</strong> — a number that scales a connection between two neurons. Positive weights amplify signals; negative weights suppress them. Training evolves these values.</p>
        <p><strong className="text-foreground">Bias</strong> — an offset added to a neuron's output before activation. Allows the network to shift its decision boundary independently of inputs.</p>
        <p><strong className="text-foreground">Mutation (μ / σ)</strong> — μ (mu) is the mutation rate — the probability a weight is changed each generation. σ (sigma) is the standard deviation of the Gaussian noise added to mutated weights. Both adapt conservatively (σ ≤ 0.5, μ ≤ 10%). On prolonged stagnation, 20% of the population is replaced with fresh random individuals.</p>
        <p><strong className="text-foreground">SBX (η)</strong> — Simulated Binary Crossover. Combines two parent genomes to produce offspring. η (eta) is the distribution index: higher values (η=20) keep offspring close to parents (exploitation), lower values allow more exploration. Unlike BLX-α, SBX preserves locality in high-dimensional weight spaces.</p>
        <p><strong className="text-foreground">Edge colors</strong> — connections between layers show average weights. <span className="text-blue-500 font-semibold">Blue = positive</span> (amplifying), <span className="text-red-500 font-semibold">Red = negative</span> (suppressing). Thicker = stronger. Each edge averages all weights between its source group and destination layer.</p>
      </div>

      {/* Reset confirmation dialog */}
      <Dialog open={showResetDialog} onOpenChange={setShowResetDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Start a new lineage?</DialogTitle>
            <DialogDescription>
              This will create a brand new untrained model (Generation 0) with a new lineage.
              The current model's training progress will be lost. Saved generation snapshots will be preserved.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowResetDialog(false)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleReset}>
              Reset
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// --- SVG Network Diagram ---

/**
 * Interpolate between red (-1), dark gray (0), and blue (+1).
 * `t` should be pre-normalized to [-1, 1] by the caller.
 */
function weightToColor(t: number): string {
  const neutral = [85, 85, 85]; // #555
  const positive = [59, 130, 246]; // #3b82f6
  const negative = [239, 68, 68]; // #ef4444
  const target = t >= 0 ? positive : negative;
  const absT = Math.min(Math.abs(t), 1);
  const r = Math.round(neutral[0] + (target[0] - neutral[0]) * absT);
  const g = Math.round(neutral[1] + (target[1] - neutral[1]) * absT);
  const b = Math.round(neutral[2] + (target[2] - neutral[2]) * absT);
  return `rgb(${r},${g},${b})`;
}

function NetworkDiagram({ model }: { model: GeneticModelData }) {
  const hidden1Size = model.hidden1_size || model.hidden_size;
  const hidden2Size = model.hidden2_size || model.hidden_size;
  const { inputGroups, outputGroups, edges } = useMemo(
    () => computeLayout(model),
    [model]
  );
  const [hoveredEdge, setHoveredEdge] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<{ x: number; y: number; weight: number; color: string } | null>(null);
  const [highlightCategory, setHighlightCategory] = useState<'positive' | 'negative' | 'near-zero' | null>(null);

  const maxAbsWeight = useMemo(() => {
    const allWeights = [
      ...edges.inputToHidden1.map(e => Math.abs(e.weight)),
      ...edges.hidden1ToHidden2.map(e => Math.abs(e.weight)),
      ...edges.hidden2ToOutput.map(e => Math.abs(e.weight)),
    ];
    return Math.max(...allWeights, 0.001);
  }, [edges]);

  const normalize = (w: number) => w / maxAbsWeight;

  const svgWidth = 900;
  // Match computeLayout spacing: (count - 1) * spacing, starting at y=30
  const spacing = Math.max(30, Math.min(38, 300 / Math.max(inputGroups.length, outputGroups.length)));
  const inputHeight = (inputGroups.length - 1) * spacing;
  const outputHeight = (outputGroups.length - 1) * spacing;
  const ioHeight = Math.max(inputHeight, outputHeight);
  const topPadding = 30; // same as computeLayout's inputStartY base
  // Hidden box heights proportional to neuron count
  const maxHiddenNeurons = Math.max(hidden1Size, hidden2Size);
  const hiddenMaxHeight = Math.min(ioHeight * 0.6, 300);
  const hidden1BoxHeight = hiddenMaxHeight * (hidden1Size / maxHiddenNeurons);
  const hidden2BoxHeight = hiddenMaxHeight * (hidden2Size / maxHiddenNeurons);
  // Center of the I/O columns: topPadding + ioHeight / 2
  const centerY = topPadding + ioHeight / 2;
  const hidden1BoxTop = centerY - hidden1BoxHeight / 2;
  const hidden1BoxBottom = centerY + hidden1BoxHeight / 2;
  const hidden2BoxTop = centerY - hidden2BoxHeight / 2;
  const hidden2BoxBottom = centerY + hidden2BoxHeight / 2;
  // SVG height: just enough for content + legend
  const contentBottom = topPadding + ioHeight;
  const svgHeight = contentBottom + 25; // 25px for legend below content
  const inputX = 20;
  const hidden1X = svgWidth * 0.33;
  const hidden2X = svgWidth * 0.58;
  const outputX = svgWidth - 20;

  const inputNodeCx = inputX + 155;
  const outputNodeCx = outputX - 155;

  // Distribute edge connection points evenly along hidden box heights
  const inputEdgeCount = edges.inputToHidden1.length;
  const outputEdgeCount = edges.hidden2ToOutput.length;
  const hidden1InputY = (idx: number) => {
    if (inputEdgeCount <= 1) return centerY;
    const pad = 8; // inset from box edges
    return hidden1BoxTop + pad + idx * ((hidden1BoxHeight - 2 * pad) / (inputEdgeCount - 1));
  };
  const hidden1OutputY = centerY; // single edge to hidden2
  const hidden2InputY = centerY; // single edge from hidden1
  const hidden2OutputY = (idx: number) => {
    if (outputEdgeCount <= 1) return centerY;
    const pad = 8;
    return hidden2BoxTop + pad + idx * ((hidden2BoxHeight - 2 * pad) / (outputEdgeCount - 1));
  };

  // Threshold for "near zero" classification
  const nearZeroThreshold = 0.25;

  function edgeCategory(nw: number): 'positive' | 'negative' | 'near-zero' {
    if (Math.abs(nw) < nearZeroThreshold) return 'near-zero';
    return nw > 0 ? 'positive' : 'negative';
  }

  function renderEdgeGroup(
    edgeList: Edge[],
    prefix: string,
    x1Fn: (e: Edge, i: number) => number,
    y1Fn: (e: Edge, i: number) => number,
    x2Fn: (e: Edge, i: number) => number,
    y2Fn: (e: Edge, i: number) => number,
  ) {
    return edgeList.map((e, i) => {
      const key = `${prefix}-${i}`;
      const x1 = x1Fn(e, i), y1 = y1Fn(e, i), x2 = x2Fn(e, i), y2 = y2Fn(e, i);
      const nw = normalize(e.weight);
      const color = weightToColor(nw);
      const width = Math.max(1.5, Math.min(Math.abs(nw) * 5, 5));
      const isHovered = hoveredEdge === key;
      const cat = edgeCategory(nw);
      const dimmed = highlightCategory !== null && cat !== highlightCategory;
      const highlighted = highlightCategory !== null && cat === highlightCategory;
      return (
        <g key={key}>
          <line
            x1={x1} y1={y1} x2={x2} y2={y2}
            stroke="transparent" strokeWidth={12}
            onMouseEnter={() => { setHoveredEdge(key); setTooltip({ x: (x1 + x2) / 2, y: (y1 + y2) / 2, weight: e.weight, color }); }}
            onMouseLeave={() => { setHoveredEdge(null); setTooltip(null); }}
            style={{ cursor: 'crosshair' }}
          />
          <line
            x1={x1} y1={y1} x2={x2} y2={y2}
            stroke={color}
            strokeWidth={isHovered ? width + 2 : highlighted ? width + 1 : width}
            strokeOpacity={dimmed ? 0.1 : 1}
            pointerEvents="none"
            style={{ transition: 'stroke-opacity 0.15s ease-out, stroke-width 0.15s ease-out' }}
          />
        </g>
      );
    });
  }

  return (
    <div className="w-full overflow-x-auto">
      <svg
        viewBox={`0 0 ${svgWidth} ${svgHeight}`}
        className="w-full max-w-[900px] min-w-[600px] mx-auto"
        style={{ minHeight: 200 }}
      >
        {/* Edges: input → hidden1 (distributed along hidden1 box height) */}
        {renderEdgeGroup(
          edges.inputToHidden1, 'ih1',
          () => inputNodeCx, e => e.fromY!, () => hidden1X - 40, (_e, i) => hidden1InputY(i)
        )}

        {/* Edges: hidden1 → hidden2 */}
        {renderEdgeGroup(
          edges.hidden1ToHidden2, 'h1h2',
          () => hidden1X + 40, () => hidden1OutputY, () => hidden2X - 40, () => hidden2InputY
        )}

        {/* Edges: hidden2 → output (distributed along hidden2 box height) */}
        {renderEdgeGroup(
          edges.hidden2ToOutput, 'h2o',
          () => hidden2X + 40, (_e, i) => hidden2OutputY(i), () => outputNodeCx, e => e.toY!
        )}

        {/* Input group labels */}
        {inputGroups.map((g, i) => (
          <g
            key={`in-${i}`}
            className="group cursor-pointer"
            onMouseEnter={(e) => {
              const circle = e.currentTarget.querySelector('circle');
              const text = e.currentTarget.querySelector('text');
              if (circle) {
                circle.style.transform = 'scale(1.3)';
                circle.style.transformOrigin = `${inputNodeCx}px ${g.y}px`;
                circle.style.transition = 'transform 0.15s ease-out';
              }
              if (text) {
                text.style.fontWeight = '700';
                text.style.transition = 'font-weight 0.15s ease-out';
              }
            }}
            onMouseLeave={(e) => {
              const circle = e.currentTarget.querySelector('circle');
              const text = e.currentTarget.querySelector('text');
              if (circle) circle.style.transform = 'scale(1)';
              if (text) text.style.fontWeight = '';
            }}
          >
            <circle cx={inputNodeCx} cy={g.y} r={8.5} fill="#6366f1" />
            <text x={inputX} y={g.y + 4} className="fill-current text-muted-foreground" fontSize={10} textAnchor="start">
              {g.label}
            </text>
          </g>
        ))}

        {/* Hidden layer 1 box */}
        <g
          className="group cursor-pointer"
          onMouseEnter={(e) => {
            const rect = e.currentTarget.querySelector('rect');
            if (rect) {
              rect.style.transform = 'scale(1.05)';
              rect.style.transformOrigin = `${hidden1X}px ${centerY}px`;
              rect.style.transition = 'transform 0.15s ease-out';
            }
            e.currentTarget.querySelectorAll('text').forEach(t => {
              t.style.fontWeight = '700';
              t.style.transition = 'font-weight 0.15s ease-out';
            });
          }}
          onMouseLeave={(e) => {
            const rect = e.currentTarget.querySelector('rect');
            if (rect) rect.style.transform = 'scale(1)';
            e.currentTarget.querySelectorAll('text').forEach(t => {
              t.style.fontWeight = '';
            });
          }}
        >
          <rect
            x={hidden1X - 40} y={hidden1BoxTop} width={80} height={hidden1BoxBottom - hidden1BoxTop}
            rx={8} fill="none" stroke="currentColor" strokeOpacity={0.3} strokeWidth={1.5} strokeDasharray="4 2"
          />
          <text x={hidden1X} y={centerY - 8} textAnchor="middle" className="fill-current text-muted-foreground" fontSize={10}>
            {hidden1Size}
          </text>
          <text x={hidden1X} y={centerY + 6} textAnchor="middle" className="fill-current text-muted-foreground" fontSize={10}>
            neurons
          </text>
          <text x={hidden1X} y={centerY + 18} textAnchor="middle" className="fill-current text-muted-foreground" fontSize={9} fontStyle="italic">
            (ReLU)
          </text>
        </g>

        {/* Hidden layer 2 box */}
        <g
          className="group cursor-pointer"
          onMouseEnter={(e) => {
            const rect = e.currentTarget.querySelector('rect');
            if (rect) {
              rect.style.transform = 'scale(1.05)';
              rect.style.transformOrigin = `${hidden2X}px ${centerY}px`;
              rect.style.transition = 'transform 0.15s ease-out';
            }
            e.currentTarget.querySelectorAll('text').forEach(t => {
              t.style.fontWeight = '700';
              t.style.transition = 'font-weight 0.15s ease-out';
            });
          }}
          onMouseLeave={(e) => {
            const rect = e.currentTarget.querySelector('rect');
            if (rect) rect.style.transform = 'scale(1)';
            e.currentTarget.querySelectorAll('text').forEach(t => {
              t.style.fontWeight = '';
            });
          }}
        >
          <rect
            x={hidden2X - 40} y={hidden2BoxTop} width={80} height={hidden2BoxBottom - hidden2BoxTop}
            rx={8} fill="none" stroke="currentColor" strokeOpacity={0.3} strokeWidth={1.5} strokeDasharray="4 2"
          />
          <text x={hidden2X} y={centerY - 8} textAnchor="middle" className="fill-current text-muted-foreground" fontSize={10}>
            {hidden2Size}
          </text>
          <text x={hidden2X} y={centerY + 6} textAnchor="middle" className="fill-current text-muted-foreground" fontSize={10}>
            neurons
          </text>
          <text x={hidden2X} y={centerY + 18} textAnchor="middle" className="fill-current text-muted-foreground" fontSize={9} fontStyle="italic">
            (ReLU)
          </text>
        </g>

        {/* Output group labels */}
        {outputGroups.map((g, i) => (
          <g
            key={`out-${i}`}
            className="group cursor-pointer"
            onMouseEnter={(e) => {
              const circle = e.currentTarget.querySelector('circle');
              const text = e.currentTarget.querySelector('text');
              if (circle) {
                circle.style.transform = 'scale(1.3)';
                circle.style.transformOrigin = `${outputNodeCx}px ${g.y}px`;
                circle.style.transition = 'transform 0.15s ease-out';
              }
              if (text) {
                text.style.fontWeight = '700';
                text.style.transition = 'font-weight 0.15s ease-out';
              }
            }}
            onMouseLeave={(e) => {
              const circle = e.currentTarget.querySelector('circle');
              const text = e.currentTarget.querySelector('text');
              if (circle) circle.style.transform = 'scale(1)';
              if (text) text.style.fontWeight = '';
            }}
          >
            <circle cx={outputNodeCx} cy={g.y} r={8.5} fill="#f59e0b" />
            <text x={outputX} y={g.y + 4} className="fill-current text-muted-foreground" fontSize={10} textAnchor="end">
              {g.label}
            </text>
          </g>
        ))}

        {/* Legend — interactive: hover to highlight matching edges */}
        <g transform={`translate(${svgWidth / 2 - 80}, ${contentBottom + 12})`}>
          <g
            style={{ cursor: 'pointer' }}
            onMouseEnter={() => setHighlightCategory('positive')}
            onMouseLeave={() => setHighlightCategory(null)}
          >
            <rect x={-4} y={-8} width={68} height={16} fill="transparent" />
            <line x1={0} y1={0} x2={20} y2={0} stroke="#3b82f6" strokeWidth={3} />
            <text x={24} y={4} fontSize={9} className="fill-current text-muted-foreground" fontWeight={highlightCategory === 'positive' ? 700 : 400}>Positive</text>
          </g>
          <g
            style={{ cursor: 'pointer' }}
            onMouseEnter={() => setHighlightCategory('near-zero')}
            onMouseLeave={() => setHighlightCategory(null)}
          >
            <rect x={66} y={-8} width={78} height={16} fill="transparent" />
            <line x1={70} y1={0} x2={90} y2={0} stroke="rgb(85,85,85)" strokeWidth={3} />
            <text x={94} y={4} fontSize={9} className="fill-current text-muted-foreground" fontWeight={highlightCategory === 'near-zero' ? 700 : 400}>Near zero</text>
          </g>
          <g
            style={{ cursor: 'pointer' }}
            onMouseEnter={() => setHighlightCategory('negative')}
            onMouseLeave={() => setHighlightCategory(null)}
          >
            <rect x={144} y={-8} width={72} height={16} fill="transparent" />
            <line x1={148} y1={0} x2={168} y2={0} stroke="#ef4444" strokeWidth={3} />
            <text x={172} y={4} fontSize={9} className="fill-current text-muted-foreground" fontWeight={highlightCategory === 'negative' ? 700 : 400}>Negative</text>
          </g>
        </g>

        {/* Hover tooltip */}
        {tooltip && (
          <g pointerEvents="none">
            <rect
              x={tooltip.x - 26} y={tooltip.y - 20} width={52} height={18} rx={4}
              fill="var(--background, white)" stroke={tooltip.color} strokeWidth={1}
              filter="drop-shadow(0 2px 4px rgba(0,0,0,0.25))"
            />
            <text x={tooltip.x} y={tooltip.y - 8} textAnchor="middle" fontSize={11} fontWeight="bold" fill={tooltip.color}>
              {tooltip.weight.toFixed(3)}
            </text>
          </g>
        )}
      </svg>
      <p className="text-xs text-muted-foreground sm:hidden mt-1">← Scroll horizontally to see full network →</p>
      <p className="text-xs text-muted-foreground italic mt-2">
        Connections show the <strong>average weight</strong> across all neurons in each group.
        For example, the edge from &quot;Board Slots (12×3)&quot; to Hidden Layer 1 averages all 36×{hidden1Size} = {36 * hidden1Size} individual weights into a single line.
        The connection between hidden layers averages all {hidden1Size}×{hidden2Size} = {hidden1Size * hidden2Size} weights.
      </p>
    </div>
  );
}

// --- Layout computation ---

interface GroupNode {
  label: string;
  y: number;
}

interface Edge {
  fromY?: number;
  toY?: number;
  weight: number;
}

function computeLayout(model: GeneticModelData) {
  const { input_groups, output_groups, best_genome } = model;
  const inputSize = model.input_size;
  const hidden1Size = model.hidden1_size || model.hidden_size;
  const hidden2Size = model.hidden2_size || model.hidden_size;

  const totalInputGroups = input_groups.length;
  const totalOutputGroups = output_groups.length;
  const maxGroups = Math.max(totalInputGroups, totalOutputGroups);
  const spacing = Math.max(30, Math.min(38, 300 / maxGroups));

  const inputHeight = (totalInputGroups - 1) * spacing;
  const outputHeight = (totalOutputGroups - 1) * spacing;
  const maxColHeight = Math.max(inputHeight, outputHeight);
  const inputYOffset = (maxColHeight - inputHeight) / 2;
  const outputYOffset = (maxColHeight - outputHeight) / 2;
  const inputStartY = 30 + inputYOffset;
  const outputStartY = 30 + outputYOffset;

  const inputGroupNodes: GroupNode[] = input_groups.map(([label], i) => ({
    label,
    y: inputStartY + i * spacing,
  }));

  const outputGroupNodes: GroupNode[] = output_groups.map(([label], i) => ({
    label,
    y: outputStartY + i * spacing,
  }));

  // Genome layout: [W_ih1, b_h1, W_h1h2, b_h2, W_h2o, b_o]
  const w1Offset = 0;
  const b1Offset = inputSize * hidden1Size;
  const w2Offset = b1Offset + hidden1Size;
  const b2Offset = w2Offset + hidden1Size * hidden2Size;
  const w3Offset = b2Offset + hidden2Size;

  // Input → Hidden1: avg weight per input group
  const inputToHidden1: Edge[] = input_groups.map(([, start, end], gi) => {
    let weightSum = 0;
    let count = 0;
    for (let j = 0; j < hidden1Size; j++) {
      for (let i = start; i < end; i++) {
        const w = best_genome[w1Offset + j * inputSize + i];
        weightSum += w;
        count++;
      }
    }
    return { fromY: inputGroupNodes[gi].y, weight: count > 0 ? weightSum / count : 0 };
  });

  // Hidden1 → Hidden2: single aggregated edge (avg of all weights)
  const hidden1ToHidden2: Edge[] = (() => {
    let weightSum = 0;
    let count = 0;
    for (let k = 0; k < hidden2Size; k++) {
      for (let j = 0; j < hidden1Size; j++) {
        const w = best_genome[w2Offset + k * hidden1Size + j];
        weightSum += w;
        count++;
      }
    }
    return [{ weight: count > 0 ? weightSum / count : 0 }];
  })();

  // Hidden2 → Output: avg weight per output group
  const hidden2ToOutput: Edge[] = output_groups.map(([, start, end], gi) => {
    let weightSum = 0;
    let count = 0;
    for (let k = start; k < end; k++) {
      for (let j = 0; j < hidden2Size; j++) {
        const w = best_genome[w3Offset + k * hidden2Size + j];
        weightSum += w;
        count++;
      }
    }
    return { toY: outputGroupNodes[gi].y, weight: count > 0 ? weightSum / count : 0 };
  });

  return {
    inputGroups: inputGroupNodes,
    outputGroups: outputGroupNodes,
    edges: { inputToHidden1, hidden1ToHidden2, hidden2ToOutput },
  };
}
