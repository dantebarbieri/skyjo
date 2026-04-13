import init, {
  get_available_strategies,
  get_available_rules,
} from '../pkg/skyjo_wasm.js';
import type { GameHistory, ProgressStats, SimConfig, WorkerResponse } from './types';
import { buildAllSteps, renderReplayStep, type ReplayStep } from './replay';

let strategies: string[] = [];
let rules: string[] = [];
let worker: Worker | null = null;
let simRunning = false;
let simPaused = false;
let currentHistories: GameHistory[] = [];

const $ = (sel: string) => document.querySelector(sel)!;

async function main() {
  await init();

  strategies = JSON.parse(get_available_strategies());
  rules = JSON.parse(get_available_rules());

  setupForm();
}

function setupForm() {
  const playerCountSelect = $('#player-count') as HTMLSelectElement;
  const rulesSelect = $('#rules-select') as HTMLSelectElement;

  for (const rule of rules) {
    const opt = document.createElement('option');
    opt.value = rule;
    opt.textContent = rule;
    rulesSelect.appendChild(opt);
  }

  updateStrategySelects(parseInt(playerCountSelect.value));

  playerCountSelect.addEventListener('change', () => {
    updateStrategySelects(parseInt(playerCountSelect.value));
  });

  $('#btn-simulate').addEventListener('click', () => startSimulation(false));
  $('#btn-simulate-histories').addEventListener('click', () => startSimulation(true));
  $('#btn-pause').addEventListener('click', togglePause);
  $('#btn-stop').addEventListener('click', stopSimulation);
}

function updateStrategySelects(count: number) {
  const container = $('#strategy-selects')!;
  container.innerHTML = '';

  for (let i = 0; i < count; i++) {
    const div = document.createElement('div');
    div.className = 'strategy-select-row';

    const label = document.createElement('label');
    label.textContent = `Player ${i + 1}: `;
    label.htmlFor = `strategy-${i}`;

    const select = document.createElement('select');
    select.id = `strategy-${i}`;
    select.name = `strategy-${i}`;
    for (const strat of strategies) {
      const opt = document.createElement('option');
      opt.value = strat;
      opt.textContent = strat;
      select.appendChild(opt);
    }
    if (strategies.length > 1 && i % 2 === 1) {
      select.value = strategies[1];
    }

    div.appendChild(label);
    div.appendChild(select);
    container.appendChild(div);
  }
}

function getSimConfig(withHistories: boolean): SimConfig {
  const numGames = parseInt((document.getElementById('num-games') as HTMLInputElement).value);
  const seed = parseInt((document.getElementById('seed') as HTMLInputElement).value);
  const rulesName = (document.getElementById('rules-select') as HTMLSelectElement).value;
  const playerCount = parseInt(($('#player-count') as HTMLSelectElement).value);

  const strats: string[] = [];
  for (let i = 0; i < playerCount; i++) {
    const select = document.getElementById(`strategy-${i}`) as HTMLSelectElement;
    strats.push(select.value);
  }

  const maxTurns = parseInt((document.getElementById('max-turns') as HTMLInputElement).value);

  return {
    num_games: numGames,
    seed,
    strategies: strats,
    rules: rulesName,
    withHistories,
    maxTurnsPerRound: maxTurns,
  };
}

function startSimulation(withHistories: boolean) {
  if (simRunning) return;

  const config = getSimConfig(withHistories);
  const errorDiv = $('#error-display') as HTMLElement;
  errorDiv.textContent = '';
  errorDiv.hidden = true;

  // Reset UI
  currentHistories = [];
  ($('#game-list-section') as HTMLElement).hidden = true;
  ($('#replay-section') as HTMLElement).hidden = true;
  ($('#stats-section') as HTMLElement).hidden = false;
  ($('#progress-section') as HTMLElement).hidden = false;
  ($('#realtime-section') as HTMLElement).hidden = false;

  // Clear stats table
  ($('#stats-table-body') as HTMLElement).innerHTML = '';
  ($('#stat-num-games') as HTMLElement).textContent = '0';
  ($('#stat-avg-rounds') as HTMLElement).textContent = '-';
  ($('#stat-avg-turns') as HTMLElement).textContent = '-';

  setSimState(true, false);
  updateProgress(0, config.num_games, 0);

  // Create worker
  worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });

  worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
    const msg = e.data;
    switch (msg.type) {
      case 'ready':
        worker!.postMessage({ type: 'start', config });
        break;

      case 'progress':
        renderStats(msg.stats);
        updateProgress(msg.gamesCompleted, msg.totalGames, msg.elapsedMs);
        break;

      case 'complete':
        renderStats(msg.stats);
        updateProgress(msg.gamesCompleted, msg.totalGames, msg.elapsedMs);
        if (msg.histories) {
          currentHistories = msg.histories;
          renderGameList(msg.histories);
        }
        setSimState(false, false);
        cleanupWorker();
        break;

      case 'error':
        showError(msg.message);
        setSimState(false, false);
        cleanupWorker();
        break;
    }
  };

  worker.onerror = (e) => {
    showError(`Worker error: ${e.message}`);
    setSimState(false, false);
    cleanupWorker();
  };
}

function togglePause() {
  if (!simRunning || !worker) return;

  if (simPaused) {
    worker.postMessage({ type: 'resume' });
    setSimState(true, false);
  } else {
    worker.postMessage({ type: 'pause' });
    setSimState(true, true);
  }
}

function stopSimulation() {
  if (!worker) return;
  worker.postMessage({ type: 'stop' });
  // Worker will send 'complete' with partial results
}

function cleanupWorker() {
  if (worker) {
    worker.terminate();
    worker = null;
  }
}

function setSimState(running: boolean, paused: boolean) {
  simRunning = running;
  simPaused = paused;

  const btnSim = $('#btn-simulate') as HTMLButtonElement;
  const btnHist = $('#btn-simulate-histories') as HTMLButtonElement;
  const btnPause = $('#btn-pause') as HTMLButtonElement;
  const btnStop = $('#btn-stop') as HTMLButtonElement;

  btnSim.disabled = running;
  btnHist.disabled = running;
  btnPause.disabled = !running;
  btnStop.disabled = !running;

  btnPause.textContent = paused ? 'Resume' : 'Pause';

  const statusEl = $('#sim-status') as HTMLElement;
  if (!running) {
    statusEl.textContent = 'Complete';
    statusEl.className = 'status-complete';
  } else if (paused) {
    statusEl.textContent = 'Paused';
    statusEl.className = 'status-paused';
  } else {
    statusEl.textContent = 'Running...';
    statusEl.className = 'status-running';
  }
}

function updateProgress(completed: number, total: number, elapsedMs: number) {
  const pct = total > 0 ? (completed / total) * 100 : 0;
  ($('#progress-bar-fill') as HTMLElement).style.width = `${pct}%`;
  ($('#progress-text') as HTMLElement).textContent =
    `${completed} / ${total} games (${pct.toFixed(1)}%)`;

  const elapsedSec = elapsedMs / 1000;
  ($('#elapsed-time') as HTMLElement).textContent = formatDuration(elapsedMs);

  if (completed > 0 && completed < total) {
    const msPerGame = elapsedMs / completed;
    const remainingMs = msPerGame * (total - completed);
    ($('#eta') as HTMLElement).textContent = formatDuration(remainingMs);
    ($('#games-per-sec') as HTMLElement).textContent =
      `${(completed / elapsedSec).toFixed(1)} games/sec`;
  } else if (completed >= total) {
    ($('#eta') as HTMLElement).textContent = '-';
    ($('#games-per-sec') as HTMLElement).textContent =
      elapsedSec > 0 ? `${(completed / elapsedSec).toFixed(1)} games/sec` : '-';
  }
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(1)}s`;
  const min = Math.floor(sec / 60);
  const remSec = (sec % 60).toFixed(0);
  return `${min}m ${remSec}s`;
}

function showError(msg: string) {
  const errorDiv = $('#error-display') as HTMLElement;
  errorDiv.textContent = `Error: ${msg}`;
  errorDiv.hidden = false;
}

function renderStats(stats: ProgressStats) {
  const section = $('#stats-section') as HTMLElement;
  section.hidden = false;

  const tbody = $('#stats-table-body') as HTMLTableSectionElement;
  tbody.innerHTML = '';

  for (let p = 0; p < stats.num_players; p++) {
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td>Player ${p + 1}</td>
      <td>${stats.wins_per_player[p]}</td>
      <td>${(stats.win_rate_per_player[p] * 100).toFixed(1)}%</td>
      <td>${stats.avg_score_per_player[p].toFixed(1)}</td>
      <td>${stats.min_score_per_player[p]}</td>
      <td>${stats.max_score_per_player[p]}</td>
    `;
    tbody.appendChild(tr);
  }

  ($('#stat-num-games') as HTMLElement).textContent = String(stats.num_games);
  ($('#stat-avg-rounds') as HTMLElement).textContent =
    stats.avg_rounds_per_game.toFixed(2);
  ($('#stat-avg-turns') as HTMLElement).textContent =
    stats.avg_turns_per_game.toFixed(1);
}

function renderGameList(histories: GameHistory[]) {
  const section = $('#game-list-section') as HTMLElement;
  section.hidden = false;

  const tbody = $('#game-list-body') as HTMLTableSectionElement;
  tbody.innerHTML = '';

  for (let i = 0; i < histories.length; i++) {
    const h = histories[i];
    const totalTurns = h.rounds.reduce((sum, r) => sum + r.turns.length, 0);
    const totalClears = h.rounds.reduce(
      (sum, r) =>
        sum +
        r.end_of_round_clears.length +
        r.turns.reduce((ts, t) => ts + t.column_clears.length, 0),
      0
    );
    const wasTruncated = h.rounds.some((r) => r.truncated);
    const winnerStr = h.winners.map((w) => `P${w + 1}`).join(', ');
    const scoresStr = h.final_scores
      .map((s, p) => {
        const tag = h.winners.includes(p) ? `<strong>${s}</strong>` : `${s}`;
        return tag;
      })
      .join(', ');

    const turnsDetail = h.rounds.map((r, ri) => {
      const label = `R${ri + 1}:${r.turns.length}`;
      return r.truncated ? `<span class="truncated-badge">${label}!</span>` : label;
    }).join(' ');

    const tr = document.createElement('tr');
    if (wasTruncated) tr.classList.add('truncated-row');
    tr.innerHTML = `
      <td>${i + 1}</td>
      <td>${h.seed}</td>
      <td>${h.rounds.length}</td>
      <td><span title="${turnsDetail}">${totalTurns}</span></td>
      <td>${totalClears}</td>
      <td>${winnerStr}</td>
      <td>${scoresStr}</td>
      <td>${wasTruncated ? '<span class="truncated-badge">TRUNCATED</span> ' : ''}<button class="replay-btn">Replay</button></td>
    `;
    tr.querySelector('.replay-btn')!.addEventListener('click', () => openReplay(h));
    tbody.appendChild(tr);
  }
}

function openReplay(history: GameHistory) {
  const section = $('#replay-section') as HTMLElement;
  section.hidden = false;

  const steps = buildAllSteps(history);
  let currentStep = 0;

  // Build round index: first step index for each round
  const roundStarts = buildRoundStarts(steps, history);

  const container = $('#replay-container') as HTMLElement;
  const stepLabel = $('#step-counter') as HTMLElement;
  const prevBtn = $('#btn-prev') as HTMLButtonElement;
  const nextBtn = $('#btn-next') as HTMLButtonElement;
  const roundsNav = $('#replay-rounds') as HTMLElement;

  renderRoundsNav(roundsNav, history, roundStarts);

  function render() {
    renderReplayStep(container, steps[currentStep], history.strategy_names);
    stepLabel.textContent = `Step ${currentStep + 1} / ${steps.length}`;
    prevBtn.disabled = currentStep === 0;
    nextBtn.disabled = currentStep === steps.length - 1;

    // Highlight active round button
    const activeRound = steps[currentStep].roundIndex;
    roundsNav.querySelectorAll('button').forEach((btn, i) => {
      btn.classList.toggle('active-round', i === activeRound);
    });
  }

  prevBtn.onclick = () => {
    if (currentStep > 0) {
      currentStep--;
      render();
    }
  };
  nextBtn.onclick = () => {
    if (currentStep < steps.length - 1) {
      currentStep++;
      render();
    }
  };

  render();
  section.scrollIntoView({ behavior: 'smooth' });

  function jumpToRound(roundIdx: number) {
    const start = roundStarts[roundIdx];
    if (start !== undefined) {
      currentStep = start;
      render();
    }
  }

  // Attach round jump handlers
  roundsNav.querySelectorAll('button').forEach((btn, i) => {
    btn.addEventListener('click', () => jumpToRound(i));
  });
}

function buildRoundStarts(steps: ReplayStep[], history: GameHistory): number[] {
  const starts: number[] = [];
  let lastRound = -1;
  for (let i = 0; i < steps.length; i++) {
    if (steps[i].roundIndex !== lastRound) {
      starts.push(i);
      lastRound = steps[i].roundIndex;
    }
  }
  return starts;
}

function renderRoundsNav(
  container: HTMLElement,
  history: GameHistory,
  roundStarts: number[]
) {
  container.innerHTML = '<strong>Rounds: </strong>';
  for (let i = 0; i < history.rounds.length; i++) {
    const r = history.rounds[i];
    const btn = document.createElement('button');
    const turnCount = r.turns.length;
    btn.textContent = `R${i + 1} (${turnCount} turns)`;
    if (r.truncated) {
      btn.classList.add('truncated-badge');
      btn.title = 'This round was truncated by the turn limit';
    }
    container.appendChild(btn);
  }
}

main().catch((e) => {
  console.error('Failed to initialize WASM:', e);
  document.body.innerHTML = `<p style="color:red">Failed to load WASM module: ${e}</p>`;
});
