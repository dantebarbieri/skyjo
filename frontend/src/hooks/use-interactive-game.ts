import { useState, useCallback, useEffect, useRef } from 'react';
import type {
  InteractiveGameState,
  ActionNeeded,
  PlayerAction,
  PlayConfig,
  PlayerType,
  BotActionResponse,
} from '@/types';

export type PlayPhase =
  | 'setup'
  | 'initial_flips'
  | 'playing'
  | 'round_over'
  | 'game_over';

export interface RoundRecord {
  roundNumber: number;
  roundScores: number[];
  cumulativeScores: number[];
  goingOutPlayer: number | null;
}

/** Serializable save data for localStorage and export/import */
export interface GameSaveData {
  config: PlayConfig;
  actions: PlayerAction[];
}

const STORAGE_KEY = 'skyjo-play-save';

interface UseInteractiveGame {
  phase: PlayPhase;
  gameState: InteractiveGameState | null;
  actionNeeded: ActionNeeded | null;
  error: string | null;
  roundHistory: RoundRecord[];
  showStartingPlayer: boolean;
  startingPlayerIndex: number;
  hasSavedGame: boolean;
  playerTypes: PlayerType[];
  gameId: number | null;

  createGame: (config: PlayConfig) => void;
  applyAction: (action: PlayerAction) => void;
  applyBotTurn: (strategyName: string) => void;
  continueToNextRound: () => void;
  resetGame: () => void;
  dismissStartingPlayer: () => void;
  resumeGame: () => void;
  exportGame: () => string | null;
  importGame: (json: string) => void;
}

// Module-level WASM module reference
let wasmMod: typeof import('../../pkg/skyjo_wasm.js') | null = null;

async function ensureWasm() {
  if (wasmMod) return wasmMod;
  const mod = await import('../../pkg/skyjo_wasm.js');
  await mod.default();
  wasmMod = mod;
  return mod;
}

function derivePhase(state: InteractiveGameState): PlayPhase {
  const { action_needed } = state;
  switch (action_needed.type) {
    case 'ChooseInitialFlips':
      return 'initial_flips';
    case 'ChooseDraw':
    case 'ChooseDeckDrawAction':
    case 'ChooseDiscardDrawPlacement':
      return 'playing';
    case 'RoundOver':
      return 'round_over';
    case 'GameOver':
      return 'game_over';
  }
}

/** Extract round records by replaying state transitions */
function extractRoundHistory(
  mod: typeof import('../../pkg/skyjo_wasm.js'),
  config: PlayConfig,
  actions: PlayerAction[],
): { gameId: number; state: InteractiveGameState; roundHistory: RoundRecord[] } | { error: string } {
  const resultJson = mod.create_interactive_game(
    JSON.stringify({
      num_players: config.num_players,
      player_names: config.player_names,
      rules: config.rules,
      seed: config.seed,
    })
  );
  const createResult = JSON.parse(resultJson);
  if (createResult.error) return { error: createResult.error };

  const gameId: number = createResult.game_id;
  let state: InteractiveGameState = createResult.state;
  const roundHistory: RoundRecord[] = [];

  for (const action of actions) {
    const actionResultJson = mod.apply_action(gameId, JSON.stringify(action));
    const actionResult = JSON.parse(actionResultJson);
    if (actionResult.error) {
      mod.destroy_interactive_game(gameId);
      return { error: actionResult.error };
    }
    state = actionResult.state;

    // Capture round data
    if (state.action_needed.type === 'RoundOver') {
      const a = state.action_needed;
      roundHistory.push({
        roundNumber: a.round_number,
        roundScores: a.round_scores,
        cumulativeScores: a.cumulative_scores,
        goingOutPlayer: a.going_out_player,
      });
    }
    if (state.action_needed.type === 'GameOver') {
      const a = state.action_needed;
      roundHistory.push({
        roundNumber: a.round_number,
        roundScores: a.round_scores,
        cumulativeScores: a.final_scores,
        goingOutPlayer: a.going_out_player,
      });
    }
  }

  return { gameId, state, roundHistory };
}

function loadSavedGame(): GameSaveData | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const data = JSON.parse(raw);
    if (data && data.config && Array.isArray(data.actions)) return data;
  } catch { /* ignore corrupt data */ }
  return null;
}

function saveToStorage(config: PlayConfig, actions: PlayerAction[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ config, actions }));
  } catch { /* storage full — silently fail */ }
}

function clearStorage() {
  try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
}

export function useInteractiveGame(): UseInteractiveGame {
  const [phase, setPhase] = useState<PlayPhase>('setup');
  const [gameState, setGameState] = useState<InteractiveGameState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [roundHistory, setRoundHistory] = useState<RoundRecord[]>([]);
  const [showStartingPlayer, setShowStartingPlayer] = useState(false);
  const [startingPlayerIndex, setStartingPlayerIndex] = useState(0);
  const [hasSavedGame, setHasSavedGame] = useState(() => loadSavedGame() !== null);
  const [playerTypes, setPlayerTypes] = useState<PlayerType[]>([]);
  const gameIdRef = useRef<number | null>(null);
  const configRef = useRef<PlayConfig | null>(null);
  const actionsRef = useRef<PlayerAction[]>([]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (gameIdRef.current !== null && wasmMod) {
        wasmMod.destroy_interactive_game(gameIdRef.current);
        gameIdRef.current = null;
      }
    };
  }, []);

  const autoSave = useCallback(() => {
    if (configRef.current) {
      saveToStorage(configRef.current, actionsRef.current);
      setHasSavedGame(true);
    }
  }, []);

  const createGame = useCallback(async (config: PlayConfig) => {
    try {
      setError(null);
      const mod = await ensureWasm();

      // Destroy previous game if any
      if (gameIdRef.current !== null) {
        mod.destroy_interactive_game(gameIdRef.current);
        gameIdRef.current = null;
      }

      const resultJson = mod.create_interactive_game(
        JSON.stringify({
          num_players: config.num_players,
          player_names: config.player_names,
          rules: config.rules,
          seed: config.seed,
        })
      );

      const result = JSON.parse(resultJson);
      if (result.error) {
        setError(result.error);
        return;
      }

      gameIdRef.current = result.game_id;
      configRef.current = config;
      actionsRef.current = [];
      setPlayerTypes(config.player_types);
      setGameState(result.state);
      setRoundHistory([]);
      setShowStartingPlayer(false);
      setPhase(derivePhase(result.state));

      saveToStorage(config, []);
      setHasSavedGame(true);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const applyAction = useCallback((action: PlayerAction) => {
    if (gameIdRef.current === null || !wasmMod) return;

    try {
      setError(null);
      const resultJson = wasmMod.apply_action(
        gameIdRef.current,
        JSON.stringify(action)
      );
      const result = JSON.parse(resultJson);
      if (result.error) {
        setError(result.error);
        return;
      }

      // Track action for persistence
      actionsRef.current = [...actionsRef.current, action];

      const newState: InteractiveGameState = result.state;
      const newPhase = derivePhase(newState);

      // Detect transition from initial_flips → playing (starting player popup)
      if (
        gameState &&
        gameState.action_needed.type === 'ChooseInitialFlips' &&
        newPhase === 'playing'
      ) {
        setStartingPlayerIndex(newState.current_player);
        setShowStartingPlayer(true);
      }

      // Capture round data when round ends
      if (newPhase === 'round_over' && newState.action_needed.type === 'RoundOver') {
        const a = newState.action_needed;
        setRoundHistory(prev => [...prev, {
          roundNumber: a.round_number,
          roundScores: a.round_scores,
          cumulativeScores: a.cumulative_scores,
          goingOutPlayer: a.going_out_player,
        }]);
      }

      // Capture final round data when game ends
      if (newPhase === 'game_over' && newState.action_needed.type === 'GameOver') {
        const a = newState.action_needed;
        setRoundHistory(prev => [...prev, {
          roundNumber: a.round_number,
          roundScores: a.round_scores,
          cumulativeScores: a.final_scores,
          goingOutPlayer: a.going_out_player,
        }]);
      }

      setGameState(newState);
      setPhase(newPhase);

      // Auto-save after every action
      autoSave();
    } catch (e) {
      setError(String(e));
    }
  }, [gameState, autoSave]);

  const applyBotTurn = useCallback((strategyName: string) => {
    if (gameIdRef.current === null || !wasmMod) return;

    try {
      setError(null);
      const resultJson = wasmMod.apply_bot_action(gameIdRef.current, strategyName);
      const result = JSON.parse(resultJson);
      if (result.error) {
        setError(result.error);
        return;
      }

      const botAction: PlayerAction = result.action;
      const newState: InteractiveGameState = result.state;
      const newPhase = derivePhase(newState);

      // Track the bot's action for persistence (same as human actions)
      actionsRef.current = [...actionsRef.current, botAction];

      // Detect transition from initial_flips → playing (starting player popup)
      if (
        gameState &&
        gameState.action_needed.type === 'ChooseInitialFlips' &&
        newPhase === 'playing'
      ) {
        setStartingPlayerIndex(newState.current_player);
        setShowStartingPlayer(true);
      }

      // Capture round data when round ends
      if (newPhase === 'round_over' && newState.action_needed.type === 'RoundOver') {
        const a = newState.action_needed;
        setRoundHistory(prev => [...prev, {
          roundNumber: a.round_number,
          roundScores: a.round_scores,
          cumulativeScores: a.cumulative_scores,
          goingOutPlayer: a.going_out_player,
        }]);
      }

      // Capture final round data when game ends
      if (newPhase === 'game_over' && newState.action_needed.type === 'GameOver') {
        const a = newState.action_needed;
        setRoundHistory(prev => [...prev, {
          roundNumber: a.round_number,
          roundScores: a.round_scores,
          cumulativeScores: a.final_scores,
          goingOutPlayer: a.going_out_player,
        }]);
      }

      setGameState(newState);
      setPhase(newPhase);
      autoSave();
    } catch (e) {
      setError(String(e));
    }
  }, [gameState, autoSave]);

  const continueToNextRound = useCallback(() => {
    if (gameIdRef.current === null || !wasmMod) return;

    try {
      setError(null);
      const action: PlayerAction = { type: 'ContinueToNextRound' };
      const resultJson = wasmMod.apply_action(
        gameIdRef.current,
        JSON.stringify(action)
      );
      const result = JSON.parse(resultJson);
      if (result.error) {
        setError(result.error);
        return;
      }

      actionsRef.current = [...actionsRef.current, action];

      const newState: InteractiveGameState = result.state;
      setGameState(newState);
      setPhase(derivePhase(newState));

      autoSave();
    } catch (e) {
      setError(String(e));
    }
  }, [autoSave]);

  const restoreFromSave = useCallback(async (saveData: GameSaveData) => {
    try {
      setError(null);
      const mod = await ensureWasm();

      // Destroy previous game if any
      if (gameIdRef.current !== null) {
        mod.destroy_interactive_game(gameIdRef.current);
        gameIdRef.current = null;
      }

      const result = extractRoundHistory(mod, saveData.config, saveData.actions);
      if ('error' in result) {
        setError(result.error);
        return;
      }

      gameIdRef.current = result.gameId;
      configRef.current = saveData.config;
      actionsRef.current = [...saveData.actions];
      // Backwards compat: old saves don't have player_types
      setPlayerTypes(
        saveData.config.player_types ||
        Array(saveData.config.num_players).fill('Human' as PlayerType)
      );
      setGameState(result.state);
      setRoundHistory(result.roundHistory);
      setShowStartingPlayer(false);
      setPhase(derivePhase(result.state));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const resumeGame = useCallback(() => {
    const saveData = loadSavedGame();
    if (saveData) {
      restoreFromSave(saveData);
    }
  }, [restoreFromSave]);

  const exportGame = useCallback((): string | null => {
    if (!configRef.current) return null;
    const saveData: GameSaveData = {
      config: configRef.current,
      actions: actionsRef.current,
    };
    return JSON.stringify(saveData);
  }, []);

  const importGame = useCallback((json: string) => {
    try {
      const saveData: GameSaveData = JSON.parse(json);
      if (!saveData.config || !Array.isArray(saveData.actions)) {
        setError('Invalid save data');
        return;
      }
      // Save to localStorage too
      saveToStorage(saveData.config, saveData.actions);
      setHasSavedGame(true);
      restoreFromSave(saveData);
    } catch {
      setError('Failed to parse save data');
    }
  }, [restoreFromSave]);

  const dismissStartingPlayer = useCallback(() => {
    setShowStartingPlayer(false);
  }, []);

  const resetGame = useCallback(() => {
    if (gameIdRef.current !== null && wasmMod) {
      wasmMod.destroy_interactive_game(gameIdRef.current);
      gameIdRef.current = null;
    }
    configRef.current = null;
    actionsRef.current = [];
    setPlayerTypes([]);
    setGameState(null);
    setPhase('setup');
    setError(null);
    setRoundHistory([]);
    setShowStartingPlayer(false);
    clearStorage();
    setHasSavedGame(false);
  }, []);

  return {
    phase,
    gameState,
    actionNeeded: gameState?.action_needed ?? null,
    error,
    roundHistory,
    showStartingPlayer,
    startingPlayerIndex,
    hasSavedGame,
    playerTypes,
    gameId: gameIdRef.current,
    createGame,
    applyAction,
    applyBotTurn,
    continueToNextRound,
    resetGame,
    dismissStartingPlayer,
    resumeGame,
    exportGame,
    importGame,
  };
}
