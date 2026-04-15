import { getCardColorGroup, getCardClasses, CARD_COLORS, COLUMN_CLEAR_COLORS } from '@/lib/card-styles';
import type { CardColorGroup } from '@/lib/card-styles';

describe('getCardColorGroup', () => {
  it('returns "negative" for negative values', () => {
    expect(getCardColorGroup(-2)).toBe('negative');
    expect(getCardColorGroup(-1)).toBe('negative');
  });

  it('returns "zero" for 0', () => {
    expect(getCardColorGroup(0)).toBe('zero');
  });

  it('returns "low" for 1–4, "mid" for 5–8, "high" for 9–12', () => {
    expect(getCardColorGroup(1)).toBe('low');
    expect(getCardColorGroup(4)).toBe('low');
    expect(getCardColorGroup(5)).toBe('mid');
    expect(getCardColorGroup(8)).toBe('mid');
    expect(getCardColorGroup(9)).toBe('high');
    expect(getCardColorGroup(12)).toBe('high');
  });
});

describe('getCardClasses', () => {
  it('returns correct class string for each color group', () => {
    for (const [group, colors] of Object.entries(CARD_COLORS)) {
      const sampleValue = { negative: -2, zero: 0, low: 2, mid: 6, high: 10 }[group]!;
      expect(getCardClasses(sampleValue as Parameters<typeof getCardClasses>[0])).toBe(
        `${colors.bg} ${colors.text} ${colors.border}`,
      );
    }
  });

  it('covers all boundary values', () => {
    const boundaries = [-2, -1, 0, 1, 4, 5, 8, 9, 12] as const;
    const expected = [
      'negative', 'negative', 'zero', 'low', 'low', 'mid', 'mid', 'high', 'high',
    ] as const;

    boundaries.forEach((val, i) => {
      const group = expected[i];
      const colors = CARD_COLORS[group];
      expect(getCardClasses(val)).toBe(`${colors.bg} ${colors.text} ${colors.border}`);
    });
  });
});

describe('COLUMN_CLEAR_COLORS', () => {
  const allGroups: CardColorGroup[] = ['negative', 'zero', 'low', 'mid', 'high'];

  it('has entries for all card color groups', () => {
    for (const group of allGroups) {
      expect(COLUMN_CLEAR_COLORS).toHaveProperty(group);
    }
  });

  it('each entry has base, bright, and glow properties', () => {
    for (const group of allGroups) {
      const entry = COLUMN_CLEAR_COLORS[group];
      expect(entry).toHaveProperty('base');
      expect(entry).toHaveProperty('bright');
      expect(entry).toHaveProperty('glow');
    }
  });

  it('colors are valid CSS color strings (# or rgba)', () => {
    const cssColorPattern = /^(#[0-9a-fA-F]{3,8}|rgba?\(.+\))$/;
    for (const group of allGroups) {
      const { base, bright, glow } = COLUMN_CLEAR_COLORS[group];
      expect(base).toMatch(cssColorPattern);
      expect(bright).toMatch(cssColorPattern);
      expect(glow).toMatch(cssColorPattern);
    }
  });
});
