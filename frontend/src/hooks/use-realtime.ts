import { useState, useRef, useCallback, useEffect } from 'react';
import type { GameHistory } from '../types';
import { buildAllSteps, type ReplayStep } from '@/lib/replay-engine';

export type RealtimeSpeed = 'slow' | 'normal' | 'fast';

const SPEED_MS: Record<RealtimeSpeed, number> = {
  slow: 150,
  normal: 30,
  fast: 0,
};

interface InterstitialInfo {
  gameNumber: number;
  winners: number[];
  strategyNames: string[];
  scores: number[];
}

export function useRealtime() {
  const [step, setStep] = useState<ReplayStep | null>(null);
  const [speed, setSpeed] = useState<RealtimeSpeed>('normal');
  const [gameNumber, setGameNumber] = useState(0);
  const [interstitial, setInterstitial] = useState<InterstitialInfo | null>(null);

  const stepsRef = useRef<ReplayStep[]>([]);
  const currentStepRef = useRef(0);
  const timerRef = useRef<number | null>(null);
  const speedRef = useRef<RealtimeSpeed>('normal');
  const stoppedRef = useRef(false);
  const nextGameRef = useRef<GameHistory | null>(null);
  const prefetchRequestedRef = useRef(false);
  const gameCounterRef = useRef(0);
  const strategyNamesRef = useRef<string[]>([]);
  const onNeedNextGameRef = useRef<(() => void) | null>(null);

  speedRef.current = speed;

  const getDelayMs = useCallback(() => SPEED_MS[speedRef.current], []);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const showInterstitial = useCallback(() => {
    const steps = stepsRef.current;
    const lastStep = steps[steps.length - 1];
    const scores = lastStep.state.cumulativeScores;
    const minScore = Math.min(...scores);
    const winners = scores.map((s, i) => s === minScore ? i : -1).filter(i => i >= 0);

    setInterstitial({
      gameNumber: gameCounterRef.current,
      winners,
      strategyNames: strategyNamesRef.current,
      scores,
    });

    const delay = speedRef.current === 'fast' ? 500 : speedRef.current === 'normal' ? 1000 : 2000;
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      if (stoppedRef.current) return;
      setInterstitial(null);

      if (nextGameRef.current) {
        startGame(nextGameRef.current);
      } else {
        if (!prefetchRequestedRef.current) {
          onNeedNextGameRef.current?.();
          prefetchRequestedRef.current = true;
        }
      }
    }, delay);
  }, []);

  const advance = useCallback(() => {
    timerRef.current = null;
    if (stoppedRef.current) return;

    const steps = stepsRef.current;
    const cur = currentStepRef.current;

    if (cur < steps.length - 1) {
      currentStepRef.current = cur + 1;
      setStep(steps[cur + 1]);

      if (!prefetchRequestedRef.current && cur + 1 >= steps.length * 0.8) {
        prefetchRequestedRef.current = true;
        onNeedNextGameRef.current?.();
      }

      timerRef.current = window.setTimeout(advance, getDelayMs());
    } else {
      showInterstitial();
    }
  }, [getDelayMs, showInterstitial]);

  const startGame = useCallback((history: GameHistory) => {
    stoppedRef.current = false;
    gameCounterRef.current++;
    setGameNumber(gameCounterRef.current);
    strategyNamesRef.current = history.strategy_names;
    stepsRef.current = buildAllSteps(history);
    currentStepRef.current = 0;
    prefetchRequestedRef.current = false;
    nextGameRef.current = null;
    setInterstitial(null);

    clearTimer();
    setStep(stepsRef.current[0]);
    timerRef.current = window.setTimeout(advance, getDelayMs());
  }, [clearTimer, advance, getDelayMs]);

  const loadGame = useCallback((history: GameHistory) => {
    if (timerRef.current !== null && stepsRef.current.length > 0 && currentStepRef.current < stepsRef.current.length - 1) {
      nextGameRef.current = history;
      return;
    }
    startGame(history);
  }, [startGame]);

  const stop = useCallback(() => {
    stoppedRef.current = true;
    clearTimer();
    setStep(null);
    setInterstitial(null);
  }, [clearTimer]);

  const start = useCallback(() => {
    stoppedRef.current = false;
  }, []);

  const changeSpeed = useCallback((s: RealtimeSpeed) => {
    setSpeed(s);
    speedRef.current = s;
    if (timerRef.current !== null) {
      clearTimer();
      timerRef.current = window.setTimeout(advance, SPEED_MS[s]);
    }
  }, [clearTimer, advance]);

  const setOnNeedNextGame = useCallback((cb: () => void) => {
    onNeedNextGameRef.current = cb;
  }, []);

  useEffect(() => {
    return () => {
      clearTimer();
      stoppedRef.current = true;
    };
  }, [clearTimer]);

  return {
    step,
    speed,
    gameNumber,
    interstitial,
    loadGame,
    stop,
    start,
    changeSpeed,
    setOnNeedNextGame,
    strategyNames: strategyNamesRef.current,
  };
}
