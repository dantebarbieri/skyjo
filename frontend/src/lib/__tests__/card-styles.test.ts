import { getCardColorGroup, getCardClasses, CARD_COLORS } from '@/lib/card-styles';

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
