import { useEffect, useRef, useCallback } from 'react';
import type {
  InteractiveGameState,
  PlayerType,
  BotSpeed,
} from '@/types';
import { BOT_SPEED_MS } from '@/types';
import type { PlayPhase } from './use-interactive-game';

/** Returns the strategy name if the given player is a bot, or null if human */
function getBotStrategy(playerTypes: PlayerType[], playerIndex: number): string | null {
  const type = playerTypes[playerIndex];
  if (type && type.startsWith('Bot:')) {
    return type.slice(4);
  }
  return null;
}

/** Get the player index that needs to act from the game state */
function getActivePlayer(state: InteractiveGameState): number | null {
  const { action_needed } = state;
  switch (action_needed.type) {
    case 'ChooseInitialFlips':
      return action_needed.player;
    case 'ChooseDraw':
    case 'ChooseDeckDrawAction':
    case 'ChooseDiscardDrawPlacement':
      return action_needed.player;
    case 'RoundOver':
    case 'GameOver':
      return null;
  }
}

interface UseBotTurnsOptions {
  gameState: InteractiveGameState | null;
  phase: PlayPhase;
  playerTypes: PlayerType[];
  botSpeed: BotSpeed;
  applyBotTurn: (strategyName: string) => void;
  continueToNextRound: () => void;
  showStartingPlayer: boolean;
}

/**
 * Hook that automatically plays bot turns in interactive mode.
 * Watches the game state and triggers bot actions when it's a bot's turn.
 */
export function useBotTurns({
  gameState,
  phase,
  playerTypes,
  botSpeed,
  applyBotTurn,
  continueToNextRound,
  showStartingPlayer,
}: UseBotTurnsOptions) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const hasBots = playerTypes.some((t) => t !== 'Human');
  const hasHumans = playerTypes.some((t) => t === 'Human');

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => clearTimer, [clearTimer]);

  useEffect(() => {
    if (!hasBots || !gameState) return;

    // Don't act while the starting player dialog is shown
    if (showStartingPlayer) return;

    // Handle round_over: auto-continue if all players are bots
    if (phase === 'round_over') {
      if (!hasHumans) {
        const delay = BOT_SPEED_MS[botSpeed];
        clearTimer();
        timerRef.current = setTimeout(() => {
          continueToNextRound();
        }, delay);
        return () => { clearTimer(); };
      }
      return;
    }

    // Only act during gameplay phases
    if (phase !== 'initial_flips' && phase !== 'playing') return;

    const activePlayer = getActivePlayer(gameState);
    if (activePlayer === null) return;

    const strategy = getBotStrategy(playerTypes, activePlayer);
    if (!strategy) return; // Human's turn

    const delay = BOT_SPEED_MS[botSpeed];

    clearTimer();
    timerRef.current = setTimeout(() => {
      applyBotTurn(strategy);
    }, delay);

    return () => { clearTimer(); };
  }, [
    gameState,
    phase,
    playerTypes,
    botSpeed,
    hasBots,
    hasHumans,
    showStartingPlayer,
    applyBotTurn,
    continueToNextRound,
    clearTimer,
  ]);
}
