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
import type { GeneticModelData, GeneticTrainingStatus, SavedGenerationInfo } from '@/types';

const API_BASE = '/api';

interface NeuralNetworkVizProps {
  className?: string;
}

export function NeuralNetworkViz({ className }: NeuralNetworkVizProps) {
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
  const [trainGenCount, setTrainGenCount] = useState('100');
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
      const res = await fetch(`${API_BASE}/genetic/train`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });
      if (!res.ok) {
        setTrainError(await res.text());
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
      const res = await fetch(`${API_BASE}/genetic/stop`, { method: 'POST' });
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
      const res = await fetch(`${API_BASE}/genetic/reset`, { method: 'POST' });
      if (res.ok) {
        const s: GeneticTrainingStatus = await res.json();
        setStatus(s);
        await fetchModel();
        await fetchSaved();
      } else {
        setTrainError(await res.text());
      }
    } catch {
      setTrainError('Failed to connect to server');
    }
  }

  async function handleLoadSaved(name: string) {
    if (name === '__current__') return;
    setTrainError(null);
    try {
      const res = await fetch(`${API_BASE}/genetic/load`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      });
      if (res.ok) {
        const s: GeneticTrainingStatus = await res.json();
        setStatus(s);
        await fetchModel();
      } else {
        setTrainError(await res.text());
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
      {/* 1. Training controls */}
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

            <TabsContent value="for-generations" className="flex items-center gap-2 mt-2">
              <Input
                type="number" min={1} value={trainGenCount}
                onChange={e => setTrainGenCount(e.target.value)}
                className="w-28" placeholder="100"
                disabled={isTraining}
              />
              <Button size="sm" onClick={handleTrainForGenerations} disabled={isTraining}>
                Train
              </Button>
            </TabsContent>

            <TabsContent value="until-generation" className="flex items-center gap-2 mt-2">
              <span className="text-sm text-muted-foreground shrink-0">Target:</span>
              <Input
                type="number" min={1} value={trainTargetGen}
                onChange={e => setTrainTargetGen(e.target.value)}
                className="w-28" placeholder={String((status?.generation ?? model?.generation ?? 0) + 100)}
                disabled={isTraining}
              />
              <Button size="sm" onClick={handleTrainUntilGeneration} disabled={isTraining}>
                Train
              </Button>
              <span className="text-xs text-muted-foreground shrink-0">
                Current: {status?.generation ?? model?.generation ?? 0}
              </span>
            </TabsContent>

            <TabsContent value="until-fitness" className="flex items-center gap-2 mt-2">
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
              <span className="text-xs text-muted-foreground shrink-0">(100k gen cap)</span>
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

      {/* 2. Training progress */}
      {isTraining && (
        <Card>
          <CardContent className="py-3 px-4 space-y-2">
            <div className="flex items-center justify-between text-sm">
              {isFitnessMode ? (
                <>
                  <span className="font-medium">
                    Training: Gen {status!.generation} | Fitness {status!.best_fitness.toFixed(1)} / {status!.training_target_fitness.toFixed(1)}
                  </span>
                  <span className="text-muted-foreground text-xs">
                    {gensDone} generations (max {gensTotal})
                  </span>
                </>
              ) : (
                <>
                  <span className="font-medium">
                    Training: Gen {status!.generation} / {status!.training_target_generation}
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
              {etaSec > 0 && (
                <span title={isFitnessMode ? (fitnessEtaSec > 0 ? 'Based on fitness improvement rate' : 'Based on generation safety cap') : undefined}>
                  {isFitnessMode ? '~' : ''}ETA: {formatTime(etaSec)}
                </span>
              )}
              <Button size="sm" variant="ghost" className="h-6 px-2 text-xs text-destructive hover:text-destructive" onClick={handleCancel}>
                Cancel
              </Button>
            </div>
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
        {status && status.best_fitness !== 0 && (
          <Badge variant="outline" className="text-xs">
            Fitness: {status.best_fitness.toFixed(1)}
          </Badge>
        )}
        {lineageHash && (
          <Badge variant="outline" className="text-xs">
            Lineage: <span className="font-mono">{lineageHash}</span>
          </Badge>
        )}
        <Badge variant="outline" className="text-xs">
          Architecture: {model.input_size} / {model.hidden_size} / {model.output_size}
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
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-1 text-xs text-muted-foreground">
        <p><strong className="text-foreground">Generation</strong> — one cycle of evolution. Each generation, 50 neural networks play games, and the best are selected, crossed over, and mutated to produce the next generation.</p>
        <p><strong className="text-foreground">Fitness</strong> — how well the best network performs. Equals the negative of the average game score, so less negative = better (e.g. -50 beats -200, meaning an average score of 50 vs 200).</p>
        <p><strong className="text-foreground">Inputs ({model.input_size})</strong> — what the network sees: board state, discard pile, deck size, scores, column match potential, and the drawn card value.</p>
        <p><strong className="text-foreground">Hidden ({model.hidden_size})</strong> — internal neurons that learn patterns from the inputs. Their weights are not directly interpretable.</p>
        <p><strong className="text-foreground">Outputs ({model.output_size})</strong> — decisions the network makes: which cards to flip, whether to draw from deck or discard, whether to keep or swap, and where to place cards.</p>
        <p><strong className="text-foreground">Lineage</strong> — a unique identifier for each independent training run. When you reset the model, a new lineage begins. Saved generations retain their lineage so you can compare models from different training runs.</p>
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
  const { inputGroups, outputGroups, edges } = useMemo(
    () => computeLayout(model),
    [model]
  );
  const [hoveredEdge, setHoveredEdge] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<{ x: number; y: number; weight: number; color: string } | null>(null);

  // Normalize weights: find the absolute max across all edges so the extremes
  // map to full blue/red and zero maps to gray.
  const maxAbsWeight = useMemo(() => {
    const allWeights = [
      ...edges.inputToHidden.map(e => Math.abs(e.weight)),
      ...edges.hiddenToOutput.map(e => Math.abs(e.weight)),
    ];
    return Math.max(...allWeights, 0.001); // avoid division by zero
  }, [edges]);

  const normalize = (w: number) => w / maxAbsWeight; // maps to [-1, 1]

  const svgWidth = 700;
  const svgHeight = Math.max(
    inputGroups.length * 38 + 40,
    outputGroups.length * 38 + 40,
    200
  );
  const inputX = 20;
  const hiddenX = svgWidth / 2;
  const outputX = svgWidth - 20;
  const hiddenBoxTop = 30;
  const hiddenBoxBottom = svgHeight - 30;
  const hiddenCenterY = (hiddenBoxTop + hiddenBoxBottom) / 2;

  // Node center positions — lines connect to these
  const inputNodeCx = inputX + 155;
  const outputNodeCx = outputX - 155;

  return (
    <div className="overflow-x-auto">
      <svg
        viewBox={`0 0 ${svgWidth} ${svgHeight}`}
        className="w-full max-w-[700px] mx-auto"
        style={{ minHeight: 200 }}
      >
        {/* Edges: input → hidden (rendered first so nodes draw on top) */}
        {edges.inputToHidden.map((e, i) => {
          const key = `ih-${i}`;
          const x1 = inputNodeCx;
          const y1 = e.fromY!;
          const x2 = hiddenX - 40;
          const y2 = hiddenCenterY;
          const nw = normalize(e.weight);
          const color = weightToColor(nw);
          const width = Math.max(1.5, Math.min(Math.abs(nw) * 5, 5));
          const isHovered = hoveredEdge === key;
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
                strokeWidth={isHovered ? width + 2 : width}
                pointerEvents="none"
              />
            </g>
          );
        })}

        {/* Edges: hidden → output */}
        {edges.hiddenToOutput.map((e, i) => {
          const key = `ho-${i}`;
          const x1 = hiddenX + 40;
          const y1 = hiddenCenterY;
          const x2 = outputNodeCx;
          const y2 = e.toY!;
          const nw = normalize(e.weight);
          const color = weightToColor(nw);
          const width = Math.max(1.5, Math.min(Math.abs(nw) * 5, 5));
          const isHovered = hoveredEdge === key;
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
                strokeWidth={isHovered ? width + 2 : width}
                pointerEvents="none"
              />
            </g>
          );
        })}

        {/* Input group labels */}
        {inputGroups.map((g, i) => (
          <g key={`in-${i}`}>
            <circle cx={inputX + 155} cy={g.y} r={6} fill="#6366f1" fillOpacity={0.8} />
            <text
              x={inputX}
              y={g.y + 4}
              className="fill-current text-muted-foreground"
              fontSize={10}
              textAnchor="start"
            >
              {g.label}
            </text>
          </g>
        ))}

        {/* Hidden layer box */}
        <rect
          x={hiddenX - 40}
          y={hiddenBoxTop}
          width={80}
          height={hiddenBoxBottom - hiddenBoxTop}
          rx={8}
          fill="none"
          stroke="currentColor"
          strokeOpacity={0.3}
          strokeWidth={1.5}
          strokeDasharray="4 2"
        />
        <text
          x={hiddenX}
          y={hiddenCenterY - 8}
          textAnchor="middle"
          className="fill-current text-muted-foreground"
          fontSize={10}
        >
          {model.hidden_size}
        </text>
        <text
          x={hiddenX}
          y={hiddenCenterY + 6}
          textAnchor="middle"
          className="fill-current text-muted-foreground"
          fontSize={10}
        >
          neurons
        </text>
        <text
          x={hiddenX}
          y={hiddenCenterY + 18}
          textAnchor="middle"
          className="fill-current text-muted-foreground"
          fontSize={9}
          fontStyle="italic"
        >
          (tanh)
        </text>

        {/* Output group labels */}
        {outputGroups.map((g, i) => (
          <g key={`out-${i}`}>
            <circle cx={outputX - 155} cy={g.y} r={6} fill="#f59e0b" fillOpacity={0.8} />
            <text
              x={outputX}
              y={g.y + 4}
              className="fill-current text-muted-foreground"
              fontSize={10}
              textAnchor="end"
            >
              {g.label}
            </text>
          </g>
        ))}

        {/* Legend */}
        <g transform={`translate(${svgWidth / 2 - 80}, ${svgHeight - 15})`}>
          <line x1={0} y1={0} x2={20} y2={0} stroke="#3b82f6" strokeWidth={3} />
          <text x={24} y={4} fontSize={9} className="fill-current text-muted-foreground">
            Positive
          </text>
          <line x1={70} y1={0} x2={90} y2={0} stroke="rgb(85,85,85)" strokeWidth={3} />
          <text x={94} y={4} fontSize={9} className="fill-current text-muted-foreground">
            Near zero
          </text>
          <line x1={148} y1={0} x2={168} y2={0} stroke="#ef4444" strokeWidth={3} />
          <text x={172} y={4} fontSize={9} className="fill-current text-muted-foreground">
            Negative
          </text>
        </g>

        {/* Hover tooltip — rendered last so it's on top of everything */}
        {tooltip && (
          <g pointerEvents="none">
            <rect
              x={tooltip.x - 26}
              y={tooltip.y - 20}
              width={52}
              height={18}
              rx={4}
              fill="var(--background, white)"
              stroke={tooltip.color}
              strokeWidth={1}
              filter="drop-shadow(0 2px 4px rgba(0,0,0,0.25))"
            />
            <text
              x={tooltip.x}
              y={tooltip.y - 8}
              textAnchor="middle"
              fontSize={11}
              fontWeight="bold"
              fill={tooltip.color}
            >
              {tooltip.weight.toFixed(3)}
            </text>
          </g>
        )}
      </svg>
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
  const hiddenSize = model.hidden_size;

  // Compute Y positions for input groups
  const totalInputGroups = input_groups.length;
  const totalOutputGroups = output_groups.length;
  const maxGroups = Math.max(totalInputGroups, totalOutputGroups);
  const spacing = Math.max(30, Math.min(38, 300 / maxGroups));
  const inputStartY = 30;
  const outputStartY = 30;

  const inputGroupNodes: GroupNode[] = input_groups.map(([label], i) => ({
    label,
    y: inputStartY + i * spacing,
  }));

  const outputGroupNodes: GroupNode[] = output_groups.map(([label], i) => ({
    label,
    y: outputStartY + i * spacing,
  }));

  // Compute aggregated edge weights
  // For input→hidden: average absolute weight from each input group to all hidden neurons
  const wihOffset = 0;
  const inputToHidden: Edge[] = input_groups.map(([, start, end], gi) => {
    let weightSum = 0;
    let count = 0;
    for (let j = 0; j < hiddenSize; j++) {
      for (let i = start; i < end; i++) {
        const w = best_genome[j * inputSize + i + wihOffset];
        weightSum += w;
        count++;
      }
    }
    const avg = count > 0 ? weightSum / count : 0;
    return { fromY: inputGroupNodes[gi].y, weight: avg };
  });

  // For hidden→output: average absolute weight from all hidden neurons to each output group
  const bhOffset = inputSize * hiddenSize;
  const whoOffset = bhOffset + hiddenSize;
  const hiddenToOutput: Edge[] = output_groups.map(([, start, end], gi) => {
    let weightSum = 0;
    let count = 0;
    for (let k = start; k < end; k++) {
      for (let j = 0; j < hiddenSize; j++) {
        const w = best_genome[whoOffset + k * hiddenSize + j];
        weightSum += w;
        count++;
      }
    }
    const avg = count > 0 ? weightSum / count : 0;
    return { toY: outputGroupNodes[gi].y, weight: avg };
  });

  return {
    inputGroups: inputGroupNodes,
    outputGroups: outputGroupNodes,
    edges: { inputToHidden, hiddenToOutput },
  };
}
