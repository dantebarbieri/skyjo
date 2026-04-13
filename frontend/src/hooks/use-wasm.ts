import { useState, useEffect } from 'react';

// Module-level cache so WASM functions are accessible after init
let wasmModule: typeof import('../../pkg/skyjo_wasm.js') | null = null;

export interface WasmState {
  ready: boolean;
  strategies: string[];
  rules: string[];
  error: string | null;
}

export function useWasm(): WasmState {
  const [state, setState] = useState<WasmState>({
    ready: false,
    strategies: [],
    rules: [],
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const mod = await import('../../pkg/skyjo_wasm.js');
        await mod.default();
        wasmModule = mod;
        if (cancelled) return;

        const strategies: string[] = JSON.parse(mod.get_available_strategies());
        const rules: string[] = JSON.parse(mod.get_available_rules());

        setState({ ready: true, strategies, rules, error: null });
      } catch (err) {
        if (!cancelled) {
          setState((s) => ({ ...s, error: String(err) }));
        }
      }
    }

    load();
    return () => { cancelled = true; };
  }, []);

  return state;
}

export function getRulesInfo(rulesName: string): Record<string, string> | null {
  if (!wasmModule) return null;
  try {
    return JSON.parse(wasmModule.get_rules_info(rulesName));
  } catch {
    return null;
  }
}
