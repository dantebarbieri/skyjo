import { createContext, useContext, type ReactNode } from 'react';
import { useWasm, type WasmState } from '@/hooks/use-wasm';

const WasmContext = createContext<WasmState | null>(null);

export function WasmProvider({ children }: { children: ReactNode }) {
  const wasm = useWasm();

  if (!wasm.ready) {
    return (
      <div className="min-h-screen bg-background text-foreground flex items-center justify-center">
        {wasm.error ? (
          <div className="text-destructive text-center">
            <p className="text-lg font-semibold">Failed to load WASM module</p>
            <p className="text-sm mt-2">{wasm.error}</p>
          </div>
        ) : (
          <div className="text-muted-foreground animate-pulse text-lg">
            Loading Skyjo...
          </div>
        )}
      </div>
    );
  }

  return (
    <WasmContext.Provider value={wasm}>
      {children}
    </WasmContext.Provider>
  );
}

export function useWasmContext(): WasmState {
  const ctx = useContext(WasmContext);
  if (!ctx) {
    throw new Error('useWasmContext must be used within a WasmProvider');
  }
  return ctx;
}
