import type { CardValue } from '../types';

export type CardColorGroup = 'negative' | 'zero' | 'low' | 'mid' | 'high';

export function getCardColorGroup(value: CardValue): CardColorGroup {
  if (value < 0) return 'negative';
  if (value === 0) return 'zero';
  if (value <= 4) return 'low';
  if (value <= 8) return 'mid';
  return 'high';
}

/**
 * Card color configuration matching real Skyjo cards:
 * - Negative (-2, -1): Purple/violet
 * - Zero (0): Light blue/sky
 * - Low (1-4): Green
 * - Mid (5-8): Yellow/amber
 * - High (9-12): Red
 */
export const CARD_COLORS: Record<CardColorGroup, { bg: string; text: string; border: string }> = {
  negative: { bg: 'bg-purple-600', text: 'text-white', border: 'border-purple-700' },
  zero: { bg: 'bg-sky-300', text: 'text-sky-900', border: 'border-sky-400' },
  low: { bg: 'bg-green-500', text: 'text-white', border: 'border-green-600' },
  mid: { bg: 'bg-yellow-400', text: 'text-yellow-900', border: 'border-yellow-500' },
  high: { bg: 'bg-red-500', text: 'text-white', border: 'border-red-600' },
};

/** CSS color values for column-clear border animation (keyed by color group) */
export const COLUMN_CLEAR_COLORS: Record<CardColorGroup, { base: string; bright: string; glow: string }> = {
  negative: { base: '#9333ea', bright: '#a855f7', glow: 'rgba(147,51,234,0.5)' },  // purple
  zero:     { base: '#38bdf8', bright: '#7dd3fc', glow: 'rgba(56,189,248,0.5)' },   // sky
  low:      { base: '#22c55e', bright: '#4ade80', glow: 'rgba(34,197,94,0.5)' },    // green
  mid:      { base: '#eab308', bright: '#facc15', glow: 'rgba(234,179,8,0.5)' },    // yellow
  high:     { base: '#ef4444', bright: '#f87171', glow: 'rgba(239,68,68,0.5)' },    // red
};

export function getCardClasses(value: CardValue): string {
  const group = getCardColorGroup(value);
  const colors = CARD_COLORS[group];
  return `${colors.bg} ${colors.text} ${colors.border}`;
}
