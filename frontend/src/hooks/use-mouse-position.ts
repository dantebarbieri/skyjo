import { useEffect, useRef, useCallback } from 'react';

export interface MousePosition {
  x: number;
  y: number;
}

// Singleton: one global listener, many subscribers via callbacks
type Subscriber = (pos: MousePosition) => void;
const subscribers = new Set<Subscriber>();
let listening = false;
const currentPos: MousePosition = { x: 0, y: 0 };

function ensureListener() {
  if (listening) return;
  listening = true;
  document.addEventListener('mousemove', (e) => {
    currentPos.x = e.clientX;
    currentPos.y = e.clientY;
    for (const sub of subscribers) sub(currentPos);
  }, { passive: true });
}

/**
 * Subscribe to mouse position updates without triggering React re-renders.
 * The callback receives the new position on every mousemove.
 * Uses a singleton event listener shared across all subscribers.
 */
export function useMouseSubscription(callback: Subscriber) {
  const cbRef = useRef(callback);
  cbRef.current = callback;

  useEffect(() => {
    ensureListener();
    const sub: Subscriber = (pos) => cbRef.current(pos);
    subscribers.add(sub);
    return () => { subscribers.delete(sub); };
  }, []);
}
