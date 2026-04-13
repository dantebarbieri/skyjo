import init, { simulate_one, simulate_one_with_history } from '../pkg/skyjo_wasm.js';
import type { GameHistory, GameStats, ProgressStats, SimConfig, WorkerRequest, WorkerResponse } from './types';

let paused = false;
let stopped = false;
let realtimeRequested = false;
let resumeResolver: (() => void) | null = null;

function post(msg: WorkerResponse) {
  self.postMessage(msg);
}

async function initialize() {
  const wasmUrl = new URL('../pkg/skyjo_wasm_bg.wasm', import.meta.url);
  await init(wasmUrl);
  post({ type: 'ready' });
}

function waitForResume(): Promise<void> {
  return new Promise((resolve) => {
    resumeResolver = resolve;
  });
}

async function runSimulation(config: SimConfig) {
  paused = false;
  stopped = false;
  realtimeRequested = config.realtimeVisualization;

  const numPlayers = config.strategies.length;
  const totalGames = config.num_games;

  const wins = new Array(numPlayers).fill(0);
  const scoreSums = new Array(numPlayers).fill(0);
  const minScores = new Array(numPlayers).fill(Infinity);
  const maxScores = new Array(numPlayers).fill(-Infinity);
  let totalRounds = 0;
  let totalTurns = 0;
  let gamesCompleted = 0;

  const histories: GameHistory[] = [];
  const startTime = performance.now();
  let lastProgressTime = startTime;

  for (let i = 0; i < totalGames; i++) {
    if (stopped) break;

    // Check pause state — must yield first to allow message processing
    if (paused) {
      const elapsed = performance.now() - startTime;
      post({
        type: 'progress',
        stats: buildProgressStats(numPlayers, gamesCompleted, wins, scoreSums, minScores, maxScores, totalRounds, totalTurns),
        gamesCompleted,
        totalGames,
        elapsedMs: elapsed,
      });
      await waitForResume();
      if (stopped) break;
      lastProgressTime = performance.now();
    }

    const seed = config.seed + i;
    const gameConfig = JSON.stringify({
      seed,
      strategies: config.strategies,
      rules: config.rules,
      max_turns_per_round: config.maxTurnsPerRound,
    });

    let stats: GameStats;
    const needHistory = config.withHistories || (config.realtimeVisualization && realtimeRequested);

    try {
      if (needHistory) {
        const resultJson = simulate_one_with_history(gameConfig);
        const result = JSON.parse(resultJson);
        if (result.error) {
          post({ type: 'error', message: `Game ${i + 1} (seed ${seed}): ${result.error}` });
          return;
        }
        stats = result.stats;
        if (config.withHistories) {
          histories.push(result.history);
        }
        if (config.realtimeVisualization && realtimeRequested) {
          post({ type: 'realtimeGame', history: result.history });
          realtimeRequested = false;
        }
      } else {
        const resultJson = simulate_one(gameConfig);
        const result = JSON.parse(resultJson);
        if (result.error) {
          post({ type: 'error', message: `Game ${i + 1} (seed ${seed}): ${result.error}` });
          return;
        }
        stats = result;
      }
    } catch (e) {
      post({ type: 'error', message: `Game ${i + 1} (seed ${seed}) crashed: ${e}` });
      return;
    }

    for (const w of stats.winners) {
      wins[w]++;
    }
    for (let p = 0; p < numPlayers; p++) {
      const score = stats.final_scores[p];
      scoreSums[p] += score;
      if (score < minScores[p]) minScores[p] = score;
      if (score > maxScores[p]) maxScores[p] = score;
    }
    totalRounds += stats.num_rounds;
    totalTurns += stats.total_turns;
    gamesCompleted++;

    // Post progress and yield every ~50ms to process pause/stop messages
    const now = performance.now();
    if (now - lastProgressTime >= 50) {
      const elapsed = now - startTime;
      post({
        type: 'progress',
        stats: buildProgressStats(numPlayers, gamesCompleted, wins, scoreSums, minScores, maxScores, totalRounds, totalTurns),
        gamesCompleted,
        totalGames,
        elapsedMs: elapsed,
      });
      lastProgressTime = now;
      // Always yield to let the event loop process incoming messages
      await new Promise<void>((r) => setTimeout(r, 0));
    }
  }

  const elapsed = performance.now() - startTime;
  post({
    type: 'complete',
    stats: buildProgressStats(numPlayers, gamesCompleted, wins, scoreSums, minScores, maxScores, totalRounds, totalTurns),
    gamesCompleted,
    totalGames,
    elapsedMs: elapsed,
    histories: config.withHistories ? histories : null,
  });
}

function buildProgressStats(
  numPlayers: number,
  gamesCompleted: number,
  wins: number[],
  scoreSums: number[],
  minScores: number[],
  maxScores: number[],
  totalRounds: number,
  totalTurns: number
): ProgressStats {
  const n = Math.max(gamesCompleted, 1);
  return {
    num_games: gamesCompleted,
    num_players: numPlayers,
    wins_per_player: [...wins],
    win_rate_per_player: wins.map((w) => w / n),
    avg_score_per_player: scoreSums.map((s) => s / n),
    min_score_per_player: minScores.map((m) => (m === Infinity ? 0 : m)),
    max_score_per_player: maxScores.map((m) => (m === -Infinity ? 0 : m)),
    avg_rounds_per_game: totalRounds / n,
    avg_turns_per_game: totalTurns / n,
  };
}

// Single message handler — no competing listeners
self.addEventListener('message', (e: MessageEvent<WorkerRequest>) => {
  const msg = e.data;
  switch (msg.type) {
    case 'start':
      runSimulation(msg.config);
      break;
    case 'pause':
      paused = true;
      break;
    case 'resume':
      paused = false;
      if (resumeResolver) {
        const resolve = resumeResolver;
        resumeResolver = null;
        resolve();
      }
      break;
    case 'stop':
      stopped = true;
      // Also unblock if paused
      if (resumeResolver) {
        const resolve = resumeResolver;
        resumeResolver = null;
        resolve();
      }
      break;
    case 'requestRealtimeGame':
      realtimeRequested = true;
      break;
  }
});

initialize().catch((e) => {
  post({ type: 'error', message: `WASM init failed: ${e}` });
});
