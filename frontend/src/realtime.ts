import type { GameHistory } from './types';
import { buildAllSteps, renderReplayStep } from './replay';
import type { ReplayStep } from './replay';

export type Speed = 'slow' | 'normal' | 'fast';

export class RealtimePlayer {
  private container: HTMLElement;
  private steps: ReplayStep[] = [];
  private strategyNames: string[] = [];
  private currentStep = 0;
  private timerId: number | null = null;
  private speed: Speed = 'normal';
  private onNeedNextGame: (() => void) | null = null;
  private gameCounter = 0;
  private nextGame: GameHistory | null = null;
  private prefetchRequested = false;
  private stopped = false;

  constructor(container: HTMLElement) {
    this.container = container;
  }

  setOnNeedNextGame(cb: () => void): void {
    this.onNeedNextGame = cb;
  }

  setSpeed(speed: Speed): void {
    this.speed = speed;
    // Restart timer with new speed if currently playing
    if (this.timerId !== null) {
      clearTimeout(this.timerId);
      this.timerId = window.setTimeout(() => this.advance(), this.getDelayMs());
    }
  }

  loadGame(history: GameHistory): void {
    // If we're currently playing, queue this as the next game
    if (this.timerId !== null && this.steps.length > 0 && this.currentStep < this.steps.length - 1) {
      this.nextGame = history;
      return;
    }
    this.startGame(history);
  }

  private startGame(history: GameHistory): void {
    this.gameCounter++;
    this.strategyNames = history.strategy_names;
    this.steps = buildAllSteps(history);
    this.currentStep = 0;
    this.prefetchRequested = false;
    this.nextGame = null;

    if (this.timerId !== null) {
      clearTimeout(this.timerId);
    }

    this.render();
    this.timerId = window.setTimeout(() => this.advance(), this.getDelayMs());
  }

  stop(): void {
    this.stopped = true;
    if (this.timerId !== null) {
      clearTimeout(this.timerId);
      this.timerId = null;
    }
  }

  private advance(): void {
    this.timerId = null;
    if (this.stopped) return;

    if (this.currentStep < this.steps.length - 1) {
      this.currentStep++;
      this.render();

      // Prefetch next game at ~80% through
      if (!this.prefetchRequested && this.currentStep >= this.steps.length * 0.8) {
        this.prefetchRequested = true;
        this.onNeedNextGame?.();
      }

      this.timerId = window.setTimeout(() => this.advance(), this.getDelayMs());
    } else {
      // Last step reached — show interstitial then move to next game
      this.showInterstitial();
    }
  }

  private showInterstitial(): void {
    const step = this.steps[this.currentStep];
    const state = step.state;

    // Find winner(s) — lowest cumulative score
    const scores = state.cumulativeScores;
    const minScore = Math.min(...scores);
    const winners = scores
      .map((s, i) => (s === minScore ? i : -1))
      .filter((i) => i >= 0);

    const winnerText = winners.length === 1
      ? `Winner: Player ${winners[0] + 1} (${this.strategyNames[winners[0]]})`
      : `Winners: ${winners.map((w) => `Player ${w + 1}`).join(', ')}`;

    this.container.innerHTML = '';
    const div = document.createElement('div');
    div.className = 'game-interstitial';
    div.innerHTML = `<p>Game ${this.gameCounter} complete</p><p>${winnerText}</p><p>Final scores: ${scores.join(', ')}</p>`;
    this.container.appendChild(div);

    // After a pause, start next game or request one
    const interstitialDelay = this.speed === 'fast' ? 1000 : 2000;
    this.timerId = window.setTimeout(() => {
      this.timerId = null;
      if (this.stopped) return;

      if (this.nextGame) {
        this.startGame(this.nextGame);
      } else {
        // Request next game and wait
        if (!this.prefetchRequested) {
          this.onNeedNextGame?.();
          this.prefetchRequested = true;
        }
        // Will be started when loadGame is called
      }
    }, interstitialDelay);
  }

  private render(): void {
    const step = this.steps[this.currentStep];
    renderReplayStep(this.container, step, this.strategyNames);

    // Update game counter in header
    const counterEl = document.getElementById('realtime-game-counter');
    if (counterEl) {
      counterEl.textContent = `#${this.gameCounter}`;
    }
  }

  private getDelayMs(): number {
    switch (this.speed) {
      case 'slow': return 1500;
      case 'normal': return 600;
      case 'fast': return 150;
    }
  }
}
