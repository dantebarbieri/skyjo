import { useSyncExternalStore } from 'react';

type CardSize = 'sm' | 'md' | 'lg';

interface ResponsiveCardSizes {
  board: CardSize;
  boardActive: CardSize;
  draw: CardSize;
}

const MOBILE: ResponsiveCardSizes = { board: 'sm', boardActive: 'sm', draw: 'md' };
const TABLET: ResponsiveCardSizes = { board: 'sm', boardActive: 'md', draw: 'lg' };

function getSizes(): ResponsiveCardSizes {
  if (typeof window === 'undefined') return TABLET;
  return window.innerWidth >= 480 ? TABLET : MOBILE;
}

let listeners: Array<() => void> = [];
let current = getSizes();

function subscribe(cb: () => void) {
  listeners.push(cb);
  return () => {
    listeners = listeners.filter((l) => l !== cb);
  };
}

if (typeof window !== 'undefined') {
  const mql = window.matchMedia('(min-width: 480px)');
  mql.addEventListener('change', () => {
    current = getSizes();
    listeners.forEach((l) => l());
  });
}

function getSnapshot() {
  return current;
}

export function useResponsiveCardSize(): ResponsiveCardSizes {
  return useSyncExternalStore(subscribe, getSnapshot, () => TABLET);
}
