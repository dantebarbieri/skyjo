import { useSyncExternalStore } from 'react';

type CardSize = 'sm' | 'md' | 'lg';

interface ResponsiveCardSizes {
  board: CardSize;
  boardActive: CardSize;
  draw: CardSize;
}

const MOBILE: ResponsiveCardSizes = { board: 'sm', boardActive: 'sm', draw: 'sm' };
const TABLET: ResponsiveCardSizes = { board: 'sm', boardActive: 'md', draw: 'md' };
const DESKTOP: ResponsiveCardSizes = { board: 'sm', boardActive: 'md', draw: 'lg' };

function getSizes(): ResponsiveCardSizes {
  if (typeof window === 'undefined') return DESKTOP;
  if (window.innerWidth >= 768) return DESKTOP;
  if (window.innerWidth >= 480) return TABLET;
  return MOBILE;
}

let listeners: Array<() => void> = [];
let current = getSizes();

function subscribe(cb: () => void) {
  listeners.push(cb);
  return () => {
    listeners = listeners.filter((l) => l !== cb);
  };
}

function notify() {
  current = getSizes();
  listeners.forEach((l) => l());
}

if (typeof window !== 'undefined') {
  window.matchMedia('(min-width: 480px)').addEventListener('change', notify);
  window.matchMedia('(min-width: 768px)').addEventListener('change', notify);
}

function getSnapshot() {
  return current;
}

export function useResponsiveCardSize(): ResponsiveCardSizes {
  return useSyncExternalStore(subscribe, getSnapshot, () => DESKTOP);
}
