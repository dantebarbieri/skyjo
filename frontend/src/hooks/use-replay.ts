import { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import type { GameHistory } from '../types';
import { buildAllSteps, type ReplayStep } from '@/lib/replay-engine';

export function useReplay(history: GameHistory) {
  const steps = useMemo(() => buildAllSteps(history), [history]);

  const roundStarts = useMemo(() => {
    const starts: number[] = [];
    let lastRound = -1;
    for (let i = 0; i < steps.length; i++) {
      if (steps[i].roundIndex !== lastRound) {
        starts.push(i);
        lastRound = steps[i].roundIndex;
      }
    }
    return starts;
  }, [steps]);

  const [currentStep, setCurrentStep] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(600);
  const [pauseBetweenRounds, setPauseBetweenRounds] = useState(true);

  const timerRef = useRef<number | null>(null);
  const currentStepRef = useRef(0);
  currentStepRef.current = currentStep;

  const stopAutoplay = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    setPlaying(false);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current !== null) clearTimeout(timerRef.current);
    };
  }, []);

  // Reset when history changes
  useEffect(() => {
    stopAutoplay();
    setCurrentStep(0);
  }, [history, stopAutoplay]);

  const scheduleNext = useCallback(() => {
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      const cur = currentStepRef.current;

      if (cur >= steps.length - 1) {
        setPlaying(false);
        return;
      }

      const nextRound = steps[cur + 1].roundIndex;
      if (pauseBetweenRounds && nextRound !== steps[cur].roundIndex) {
        setPlaying(false);
        return;
      }

      const next = cur + 1;
      setCurrentStep(next);
      currentStepRef.current = next;

      // Schedule the next one
      timerRef.current = window.setTimeout(function tick() {
        timerRef.current = null;
        const c = currentStepRef.current;
        if (c >= steps.length - 1) {
          setPlaying(false);
          return;
        }
        const nr = steps[c + 1].roundIndex;
        if (pauseBetweenRounds && nr !== steps[c].roundIndex) {
          setPlaying(false);
          return;
        }
        const n = c + 1;
        setCurrentStep(n);
        currentStepRef.current = n;
        timerRef.current = window.setTimeout(tick, speed);
      }, speed);
    }, speed);
  }, [steps, speed, pauseBetweenRounds]);

  const startAutoplay = useCallback(() => {
    if (currentStepRef.current >= steps.length - 1) return;
    setPlaying(true);
    scheduleNext();
  }, [steps, scheduleNext]);

  const toggleAutoplay = useCallback(() => {
    if (playing) {
      stopAutoplay();
    } else {
      startAutoplay();
    }
  }, [playing, stopAutoplay, startAutoplay]);

  const next = useCallback(() => {
    stopAutoplay();
    setCurrentStep((s) => Math.min(s + 1, steps.length - 1));
  }, [steps, stopAutoplay]);

  const prev = useCallback(() => {
    stopAutoplay();
    setCurrentStep((s) => Math.max(s - 1, 0));
  }, [stopAutoplay]);

  const jumpToRound = useCallback((roundIdx: number) => {
    stopAutoplay();
    const start = roundStarts[roundIdx];
    if (start !== undefined) setCurrentStep(start);
  }, [roundStarts, stopAutoplay]);

  const skipToRoundStart = useCallback(() => {
    stopAutoplay();
    const curRound = steps[currentStepRef.current].roundIndex;
    setCurrentStep(roundStarts[curRound]);
  }, [steps, roundStarts, stopAutoplay]);

  const skipToRoundEnd = useCallback(() => {
    stopAutoplay();
    const curRound = steps[currentStepRef.current].roundIndex;
    const nextStart = roundStarts[curRound + 1];
    setCurrentStep(nextStart !== undefined ? nextStart - 1 : steps.length - 1);
  }, [steps, roundStarts, stopAutoplay]);

  return {
    steps,
    currentStep,
    step: steps[currentStep],
    playing,
    speed,
    pauseBetweenRounds,
    roundStarts,
    totalSteps: steps.length,
    setSpeed,
    setPauseBetweenRounds,
    toggleAutoplay,
    next,
    prev,
    jumpToRound,
    skipToRoundStart,
    skipToRoundEnd,
  };
}
