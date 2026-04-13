import { useRef, useEffect, useCallback } from 'react';
import type { CardValue, Slot } from '../types';
import { cn } from '@/lib/utils';
import { getCardColorGroup, type CardColorGroup } from '@/lib/card-styles';
import { useMouseSubscription, type MousePosition } from '@/hooks/use-mouse-position';

const COLOR_CONFIG: Record<CardColorGroup, { bg: string; text: string; cornerBg: string }> = {
  negative: { bg: 'from-purple-500 to-purple-700', text: 'text-white', cornerBg: 'bg-white/90' },
  zero: { bg: 'from-sky-200 to-sky-400', text: 'text-sky-900', cornerBg: 'bg-white/90' },
  low: { bg: 'from-green-400 to-green-600', text: 'text-white', cornerBg: 'bg-white/90' },
  mid: { bg: 'from-yellow-300 to-yellow-500', text: 'text-yellow-900', cornerBg: 'bg-white/90' },
  high: { bg: 'from-red-400 to-red-600', text: 'text-white', cornerBg: 'bg-white/90' },
};

interface SkyjoCardProps {
  slot: Slot;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
  highlight?: boolean;
}

const SIZES = {
  sm: 'w-10 h-14 text-sm',
  md: 'w-14 h-20 text-xl',
  lg: 'w-18 h-26 text-3xl',
};

const CORNER_SIZES = {
  sm: 'text-[7px] w-3 h-3',
  md: 'text-[9px] w-3.5 h-3.5',
  lg: 'text-[11px] w-4 h-4',
};

const MAX_TILT = 10; // degrees
const TILT_RANGE = 600; // px from card center to reach max tilt

function useTiltEffect(ref: React.RefObject<HTMLDivElement | null>) {
  const rafId = useRef<number | null>(null);
  const pendingPos = useRef<MousePosition | null>(null);

  const applyTilt = useCallback(() => {
    rafId.current = null;
    const el = ref.current;
    const pos = pendingPos.current;
    if (!el || !pos) return;

    const rect = el.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const dx = pos.x - cx;
    const dy = pos.y - cy;

    const rotateY = Math.max(-MAX_TILT, Math.min(MAX_TILT, (dx / TILT_RANGE) * MAX_TILT));
    const rotateX = Math.max(-MAX_TILT, Math.min(MAX_TILT, -(dy / TILT_RANGE) * MAX_TILT));

    // Shadow shifts opposite to tilt (light from above-center)
    const shadowX = -rotateY * 0.4;
    const shadowY = rotateX * 0.4 + 2;
    const shadowBlur = 8 + Math.abs(rotateX) * 0.3 + Math.abs(rotateY) * 0.3;

    el.style.transform = `perspective(800px) rotateX(${rotateX.toFixed(2)}deg) rotateY(${rotateY.toFixed(2)}deg)`;
    el.style.boxShadow = `${shadowX.toFixed(1)}px ${shadowY.toFixed(1)}px ${shadowBlur.toFixed(1)}px rgba(0,0,0,0.18)`;
  }, [ref]);

  useMouseSubscription(useCallback((pos: MousePosition) => {
    pendingPos.current = pos;
    if (rafId.current === null) {
      rafId.current = requestAnimationFrame(applyTilt);
    }
  }, [applyTilt]));

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (rafId.current !== null) cancelAnimationFrame(rafId.current);
    };
  }, []);
}

export default function SkyjoCard({ slot, size = 'md', className, highlight }: SkyjoCardProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  useTiltEffect(cardRef);

  if (typeof slot === 'string') {
    // Cleared — no tilt effect
    return (
      <div
        className={cn(
          SIZES[size],
          'rounded-lg border-2 border-dashed border-muted-foreground/30 bg-muted/30 flex items-center justify-center select-none',
          className
        )}
      />
    );
  }

  if ('Hidden' in slot) {
    return (
      <div
        ref={cardRef}
        className={cn(
          SIZES[size],
          'rounded-lg border-[3px] border-white bg-gradient-to-br from-teal-600 to-teal-800 flex items-center justify-center relative overflow-hidden will-change-transform select-none cursor-default',
          highlight && 'ring-2 ring-blue-400 ring-offset-1',
          className
        )}
        style={{
          transform: 'perspective(800px) rotateX(0deg) rotateY(0deg)',
          boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
        }}
      >
        <div className="absolute inset-0 bg-hex-pattern opacity-50" />
        <span className="relative text-white font-bold text-center leading-tight" style={{ fontSize: size === 'sm' ? '0.5rem' : size === 'md' ? '0.6rem' : '0.7rem' }}>
          SKYJO
        </span>
      </div>
    );
  }

  const value = slot.Revealed;
  const group = getCardColorGroup(value);
  const colors = COLOR_CONFIG[group];
  const valueStr = String(value);

  return (
    <div
      ref={cardRef}
      className={cn(
        SIZES[size],
        `rounded-lg border-[3px] border-white bg-gradient-to-br ${colors.bg} ${colors.text} flex items-center justify-center relative overflow-hidden will-change-transform select-none cursor-default`,
        highlight && 'ring-2 ring-blue-400 ring-offset-1',
        className
      )}
      style={{
        transform: 'perspective(800px) rotateX(0deg) rotateY(0deg)',
        boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
      }}
    >
      {/* Stained-glass mosaic pattern overlay */}
      <div className="absolute inset-0 bg-hex-pattern opacity-50" />

      {/* Top-left corner number in white circle */}
      <div className={cn(
        'absolute top-0.5 left-0.5 rounded-full flex items-center justify-center font-bold',
        colors.cornerBg,
        CORNER_SIZES[size]
      )}>
        <span className="text-gray-800">{valueStr}</span>
      </div>

      {/* Bottom-right corner number in white circle, rotated */}
      <div className={cn(
        'absolute bottom-0.5 right-0.5 rounded-full flex items-center justify-center font-bold rotate-180',
        colors.cornerBg,
        CORNER_SIZES[size]
      )}>
        <span className="text-gray-800">{valueStr}</span>
      </div>

      {/* Center number */}
      <span className="relative font-bold z-10">{valueStr}</span>
    </div>
  );
}

interface PileCardProps {
  value: CardValue | null;
  label: string;
  count: number;
  hint?: string;
  size?: 'sm' | 'md' | 'lg';
}

export function PileCard({ value, label, count, hint, size = 'md' }: PileCardProps) {
  const slot: Slot = value !== null ? { Revealed: value } : 'Cleared';

  // Scale shadow to simulate physical pile thickness
  const depth = Math.min(count, 100);
  const layers = Math.min(Math.ceil(depth / 20), 5);
  const pileStyle: React.CSSProperties = count > 1 ? {
    boxShadow: Array.from({ length: layers }, (_, i) => {
      const offset = (i + 1) * 1;
      return `${offset}px ${offset}px 0 rgba(0,0,0,0.08)`;
    }).join(', '),
  } : {};

  const cardElement = value !== null ? (
    <SkyjoCard slot={{ Revealed: value }} size={size} />
  ) : count > 0 ? (
    <SkyjoCard slot={{ Hidden: 0 }} size={size} />
  ) : (
    <SkyjoCard slot={slot} size={size} />
  );

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="text-xs text-muted-foreground font-medium">
        {label} ({count})
      </div>
      <div className="rounded-lg" style={pileStyle}>
        {cardElement}
      </div>
      {hint && (
        <div className="text-[10px] text-muted-foreground italic">{hint}</div>
      )}
    </div>
  );
}
