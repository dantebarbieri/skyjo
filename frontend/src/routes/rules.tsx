import { useState, useEffect, useCallback, useRef } from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import SkyjoCard from '@/components/skyjo-card';
import ScoringSheet from '@/components/scoring-sheet';
import { cn } from '@/lib/utils';
import { useDocumentTitle } from '@/hooks/use-document-title';
import type { Slot, GameHistory } from '@/types';

// --- Table of Contents ---

const sections = [
  { id: 'overview', label: 'Overview' },
  { id: 'cards', label: 'Cards & Deck' },
  { id: 'setup', label: 'Setup' },
  { id: 'turns', label: 'Turn Flow' },
  { id: 'columns', label: 'Column Clearing' },
  { id: 'going-out', label: 'Going Out' },
  { id: 'scoring', label: 'Scoring' },
];

function TableOfContents() {
  const [active, setActive] = useState('overview');

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActive(entry.target.id);
          }
        }
      },
      { rootMargin: '-80px 0px -60% 0px', threshold: 0 }
    );

    for (const { id } of sections) {
      const el = document.getElementById(id);
      if (el) observer.observe(el);
    }

    return () => observer.disconnect();
  }, []);

  return (
    <nav className="hidden lg:block sticky top-20 w-48 shrink-0">
      <ul className="space-y-1 text-sm">
        {sections.map(({ id, label }) => (
          <li key={id}>
            <a
              href={`#${id}`}
              className={cn(
                'block px-3 py-1 rounded-md transition-colors',
                active === id
                  ? 'bg-accent text-accent-foreground font-medium'
                  : 'text-muted-foreground hover:text-foreground'
              )}
            >
              {label}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}

// --- Section: Overview ---

function OverviewSection() {
  return (
    <section id="overview" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Overview</h2>
      <Card>
        <CardContent className="pt-6 space-y-3 text-sm leading-relaxed">
          <p>
            <strong>Skyjo</strong> is a card game for 2-8 players. The goal is to have the{' '}
            <strong>lowest cumulative score</strong> across multiple rounds. Each player has a grid
            of 12 cards (3 rows x 4 columns), most of which start face-down.
          </p>
          <p>
            On each turn, you draw a card and decide where to place it or whether to flip a hidden
            card. When all of a player's cards are revealed, the round ends. The game continues
            until someone reaches 100 points — the player with the lowest total wins.
          </p>
        </CardContent>
      </Card>
    </section>
  );
}

// --- Section: Cards & Deck ---

const DECK_COMPOSITION: { value: number; count: number }[] = [
  { value: -2, count: 5 },
  { value: -1, count: 10 },
  { value: 0, count: 15 },
  ...Array.from({ length: 12 }, (_, i) => ({ value: i + 1, count: 10 })),
];

function CardsSection() {
  return (
    <section id="cards" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Cards & Deck</h2>
      <Card>
        <CardContent className="pt-6 space-y-4">
          <p className="text-sm leading-relaxed">
            The deck contains <strong>150 cards</strong> with values from <strong>-2</strong> to{' '}
            <strong>12</strong>. Lower cards are better — negative cards reduce your score!
          </p>
          <div className="grid grid-cols-5 sm:grid-cols-8 md:grid-cols-10 lg:grid-cols-15 gap-3">
            {DECK_COMPOSITION.map(({ value, count }) => (
              <div key={value} className="flex flex-col items-center gap-1">
                <SkyjoCard slot={{ Revealed: value }} size="sm" />
                <span className="text-xs text-muted-foreground font-medium">{count}x</span>
              </div>
            ))}
          </div>
          <div className="text-xs text-muted-foreground space-y-1 pt-2 border-t">
            <p><span className="inline-block w-3 h-3 rounded bg-purple-600 align-middle mr-1" /> Purple: negative values (-2, -1)</p>
            <p><span className="inline-block w-3 h-3 rounded bg-sky-300 align-middle mr-1" /> Blue: zero (0)</p>
            <p><span className="inline-block w-3 h-3 rounded bg-green-500 align-middle mr-1" /> Green: low (1-4)</p>
            <p><span className="inline-block w-3 h-3 rounded bg-yellow-400 align-middle mr-1" /> Yellow: mid (5-8)</p>
            <p><span className="inline-block w-3 h-3 rounded bg-red-500 align-middle mr-1" /> Red: high (9-12)</p>
          </div>
        </CardContent>
      </Card>
    </section>
  );
}

// --- Section: Setup Demo ---

function SetupSection() {
  const [flipped, setFlipped] = useState<Set<number>>(new Set());
  const [demoValues] = useState(() => [3, 7, -1, 11, 0, 5, 8, 2, 12, -2, 4, 9]);
  const numRows = 3;
  const numCols = 4;

  const handleFlip = useCallback((idx: number) => {
    if (flipped.size >= 2) return;
    setFlipped((prev) => {
      if (prev.has(idx)) return prev;
      const next = new Set(prev);
      next.add(idx);
      return next;
    });
  }, [flipped.size]);

  const handleReset = useCallback(() => setFlipped(new Set()), []);

  return (
    <section id="setup" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Setup</h2>
      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="text-sm leading-relaxed space-y-2">
            <p>
              Each player is dealt <strong>12 cards</strong> arranged in a 3x4 grid, all
              face-down. Then each player <strong>flips 2 cards</strong> of their choice to
              start the round.
            </p>
            <p>
              The player with the <strong>highest sum</strong> of their flipped cards goes first.
            </p>
          </div>

          <div className="flex flex-col items-center gap-3">
            <p className="text-xs text-muted-foreground">
              {flipped.size < 2
                ? `Click ${2 - flipped.size} card${flipped.size === 1 ? '' : 's'} to flip`
                : 'Both cards flipped!'}
            </p>
            <div
              className="grid gap-1.5"
              style={{ gridTemplateColumns: `repeat(${numCols}, 1fr)` }}
            >
              {Array.from({ length: numRows }, (_, r) =>
                Array.from({ length: numCols }, (_, c) => {
                  const idx = c * numRows + r; // column-major
                  const isFlipped = flipped.has(idx);
                  const slot: Slot = isFlipped
                    ? { Revealed: demoValues[idx] }
                    : { Hidden: demoValues[idx] };
                  return (
                    <button
                      key={idx}
                      onClick={() => handleFlip(idx)}
                      disabled={isFlipped || flipped.size >= 2}
                      className={cn(
                        'transition-transform',
                        !isFlipped && flipped.size < 2 && 'hover:scale-105 cursor-pointer',
                        isFlipped && 'scale-100',
                      )}
                    >
                      <SkyjoCard slot={slot} size="md" />
                    </button>
                  );
                })
              ).flat()}
            </div>
            {flipped.size === 2 && (
              <Button variant="outline" size="sm" onClick={handleReset}>
                Reset Demo
              </Button>
            )}
          </div>
        </CardContent>
      </Card>
    </section>
  );
}

// --- Section: Turn Flow ---

type TurnDemoPhase =
  | 'choose_draw'
  | 'drew_from_deck'
  | 'drew_from_discard'
  | 'placed'
  | 'discarded_and_flipped';

function TurnFlowSection() {
  const numRows = 3;
  const numCols = 4;
  // Pre-set board: a mix of revealed and hidden
  const [boardValues] = useState(() => [3, 7, -1, 11, 0, 5, 8, 2, 12, -2, 4, 9]);
  const initialRevealed = new Set([0, 4]); // two cards already flipped
  const [board, setBoard] = useState<Slot[]>(() =>
    boardValues.map((v, i) =>
      initialRevealed.has(i) ? { Revealed: v } : { Hidden: v }
    )
  );
  const [phase, setPhase] = useState<TurnDemoPhase>('choose_draw');
  const [drawnCard, setDrawnCard] = useState<number | null>(null);
  const [discardTop, setDiscardTop] = useState(6);
  const [message, setMessage] = useState('Choose: draw from the deck or the discard pile');
  const [highlightPositions, setHighlightPositions] = useState<Set<number>>(new Set());
  const [flipMode, setFlipMode] = useState(false);

  const resetDemo = useCallback(() => {
    setBoard(boardValues.map((v, i) =>
      initialRevealed.has(i) ? { Revealed: v } : { Hidden: v }
    ));
    setPhase('choose_draw');
    setDrawnCard(null);
    setDiscardTop(6);
    setMessage('Choose: draw from the deck or the discard pile');
    setHighlightPositions(new Set());
    setFlipMode(false);
  }, [boardValues]);

  const undoDraw = useCallback(() => {
    if (phase === 'drew_from_discard') {
      // Put discard draw back
      setDiscardTop(drawnCard!);
    }
    setDrawnCard(null);
    setPhase('choose_draw');
    setMessage('Choose: draw from the deck or the discard pile');
    setHighlightPositions(new Set());
    setFlipMode(false);
  }, [phase, drawnCard]);

  const handleDrawDeck = useCallback(() => {
    if (phase !== 'choose_draw') return;
    const card = 4; // pre-determined draw
    setDrawnCard(card);
    setPhase('drew_from_deck');
    setMessage('You drew a 4. Place it on your board, or discard it and flip a hidden card.');
    setHighlightPositions(new Set(board.map((_, i) => i).filter(i => board[i] !== 'Cleared')));
  }, [phase, board]);

  const handleDrawDiscard = useCallback(() => {
    if (phase !== 'choose_draw') return;
    setDrawnCard(discardTop);
    setPhase('drew_from_discard');
    setMessage(`You drew a ${discardTop} from the discard pile. You must place it on your board.`);
    setHighlightPositions(new Set(board.map((_, i) => i).filter(i => board[i] !== 'Cleared')));
  }, [phase, discardTop, board]);

  const handleDiscardAndFlip = useCallback(() => {
    if (phase !== 'drew_from_deck') return;
    setFlipMode(true);
    setMessage('Click a hidden card to flip it face-up.');
    const hiddenPositions = new Set(
      board.map((s, i) => i).filter(i => typeof board[i] === 'object' && 'Hidden' in (board[i] as object))
    );
    setHighlightPositions(hiddenPositions);
  }, [phase, board]);

  const handleBoardClick = useCallback((idx: number) => {
    if (phase === 'drew_from_deck' && !flipMode) {
      // Place drawn card, discard the replaced card
      const replaced = board[idx];
      const replacedValue = typeof replaced === 'object' && 'Revealed' in replaced
        ? replaced.Revealed
        : typeof replaced === 'object' && 'Hidden' in replaced
          ? (replaced as { Hidden: number }).Hidden
          : null;

      setBoard(prev => {
        const next = [...prev];
        next[idx] = { Revealed: drawnCard! };
        return next;
      });
      if (replacedValue !== null) setDiscardTop(replacedValue);
      setDrawnCard(null);
      setPhase('placed');
      setMessage(`Placed the ${drawnCard} and discarded the replaced card. Turn complete!`);
      setHighlightPositions(new Set());
    } else if (phase === 'drew_from_deck' && flipMode) {
      // Discard drawn card, flip hidden card
      setDiscardTop(drawnCard!);
      setBoard(prev => {
        const next = [...prev];
        const slot = next[idx];
        if (typeof slot === 'object' && 'Hidden' in slot) {
          next[idx] = { Revealed: (slot as { Hidden: number }).Hidden };
        }
        return next;
      });
      setDrawnCard(null);
      setPhase('discarded_and_flipped');
      setMessage(`Discarded the ${drawnCard} and flipped a hidden card. Turn complete!`);
      setHighlightPositions(new Set());
      setFlipMode(false);
    } else if (phase === 'drew_from_discard') {
      // Must place the discard draw
      const replaced = board[idx];
      const replacedValue = typeof replaced === 'object' && 'Revealed' in replaced
        ? replaced.Revealed
        : typeof replaced === 'object' && 'Hidden' in replaced
          ? (replaced as { Hidden: number }).Hidden
          : null;

      setBoard(prev => {
        const next = [...prev];
        next[idx] = { Revealed: drawnCard! };
        return next;
      });
      if (replacedValue !== null) setDiscardTop(replacedValue);
      setDrawnCard(null);
      setPhase('placed');
      setMessage(`Placed the ${drawnCard} from the discard pile. Turn complete!`);
      setHighlightPositions(new Set());
    }
  }, [phase, flipMode, drawnCard, board]);

  const isDone = phase === 'placed' || phase === 'discarded_and_flipped';

  return (
    <section id="turns" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Turn Flow</h2>
      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="text-sm leading-relaxed space-y-2">
            <p>On your turn, you have two options:</p>
            <ol className="list-decimal list-inside space-y-1 ml-2">
              <li>
                <strong>Draw from the deck</strong> — look at the card, then either:
                <ul className="list-disc list-inside ml-4 text-muted-foreground">
                  <li>Keep it: replace any card on your board (hidden or revealed)</li>
                  <li>Discard it: flip one of your hidden cards face-up</li>
                </ul>
              </li>
              <li>
                <strong>Draw from the discard pile</strong> — you must place it on your board
                (replace any card)
              </li>
            </ol>
          </div>

          <div className="border rounded-lg p-4 bg-muted/30 space-y-4">
            <div className="text-sm font-medium text-center">{message}</div>

            <div className="flex items-center justify-center gap-6">
              {/* Deck */}
              <button
                onClick={handleDrawDeck}
                disabled={phase !== 'choose_draw'}
                className={cn(
                  'flex flex-col items-center gap-1 transition-transform',
                  phase === 'choose_draw' && 'hover:scale-105 cursor-pointer',
                )}
              >
                <span className="text-xs text-muted-foreground">Deck</span>
                <div className={cn(phase === 'choose_draw' && 'ring-2 ring-blue-400 rounded-lg')}>
                  <SkyjoCard slot={{ Hidden: 0 }} size="md" />
                </div>
              </button>

              {/* Discard */}
              <button
                onClick={handleDrawDiscard}
                disabled={phase !== 'choose_draw'}
                className={cn(
                  'flex flex-col items-center gap-1 transition-transform',
                  phase === 'choose_draw' && 'hover:scale-105 cursor-pointer',
                )}
              >
                <span className="text-xs text-muted-foreground">Discard</span>
                <div className={cn(phase === 'choose_draw' && 'ring-2 ring-blue-400 rounded-lg')}>
                  <SkyjoCard slot={{ Revealed: discardTop }} size="md" />
                </div>
              </button>

              {/* Drawn card */}
              {drawnCard !== null && (
                <div className="flex flex-col items-center gap-1">
                  <span className="text-xs text-muted-foreground">Drawn</span>
                  <div className="ring-2 ring-green-400 rounded-lg">
                    <SkyjoCard slot={{ Revealed: drawnCard }} size="md" />
                  </div>
                </div>
              )}
            </div>

            {phase === 'drew_from_deck' && !flipMode && (
              <div className="flex justify-center gap-2">
                <Button variant="outline" size="sm" onClick={handleDiscardAndFlip}>
                  Discard & Flip Instead
                </Button>
                <Button variant="outline" size="sm" onClick={undoDraw}>
                  Undo — Draw From Discard Instead
                </Button>
              </div>
            )}

            {phase === 'drew_from_deck' && flipMode && (
              <div className="flex justify-center">
                <Button variant="outline" size="sm" onClick={undoDraw}>
                  Undo — Choose Again
                </Button>
              </div>
            )}

            {phase === 'drew_from_discard' && (
              <div className="flex justify-center">
                <Button variant="outline" size="sm" onClick={undoDraw}>
                  Undo — Put Back & Choose Again
                </Button>
              </div>
            )}

            {/* Board */}
            <div className="flex flex-col items-center gap-2">
              <span className="text-xs text-muted-foreground">Your Board</span>
              <div
                className="grid gap-1.5"
                style={{ gridTemplateColumns: `repeat(${numCols}, 1fr)` }}
              >
                {Array.from({ length: numRows }, (_, r) =>
                  Array.from({ length: numCols }, (_, c) => {
                    const idx = c * numRows + r;
                    const slot = board[idx];
                    const canClick =
                      highlightPositions.has(idx) &&
                      (phase === 'drew_from_deck' || phase === 'drew_from_discard');
                    return (
                      <button
                        key={idx}
                        onClick={() => canClick && handleBoardClick(idx)}
                        disabled={!canClick}
                        className={cn(
                          'transition-transform',
                          canClick && 'hover:scale-105 cursor-pointer',
                        )}
                      >
                        <SkyjoCard
                          slot={slot}
                          size="md"
                          highlight={highlightPositions.has(idx) && canClick}
                        />
                      </button>
                    );
                  })
                ).flat()}
              </div>
            </div>

            {isDone && (
              <div className="flex justify-center">
                <Button variant="outline" size="sm" onClick={resetDemo}>
                  Try Again
                </Button>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </section>
  );
}

// --- Section: Column Clearing ---

function ColumnClearSection() {
  const numRows = 3;
  const numCols = 4;
  const [phase, setPhase] = useState<'before' | 'matched' | 'cleared'>('before');
  // Board where column 1 (indices 3,4,5) has two 5s revealed and one hidden 5
  const boardValues = [3, 7, -1, 5, 5, 5, 8, 2, 12, -2, 4, 9];
  const initialRevealed = new Set([0, 3, 4, 6, 9, 10]); // col 1 has idx 3,4 revealed, 5 hidden

  const getBoard = useCallback((): Slot[] => {
    return boardValues.map((v, i) => {
      if (phase === 'cleared' && (i === 3 || i === 4 || i === 5)) {
        return 'Cleared';
      }
      if (phase === 'matched' && i === 5) {
        return { Revealed: v }; // flipped to show match
      }
      return initialRevealed.has(i) ? { Revealed: v } : { Hidden: v };
    });
  }, [phase]);

  const board = getBoard();

  const handleStep = useCallback(() => {
    if (phase === 'before') setPhase('matched');
    else if (phase === 'matched') setPhase('cleared');
  }, [phase]);

  const handleReset = useCallback(() => setPhase('before'), []);

  const stepLabel = phase === 'before'
    ? 'Flip Hidden Card'
    : phase === 'matched'
      ? 'Clear Column'
      : null;

  return (
    <section id="columns" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Column Clearing</h2>
      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="text-sm leading-relaxed space-y-2">
            <p>
              When all cards in a column are <strong>revealed</strong> and have the{' '}
              <strong>same value</strong>, the entire column is <strong>discarded</strong>. This
              removes those cards from your score — a powerful way to reduce points!
            </p>
            <p>Column clearing can happen during your turn or at the end of a round.</p>
          </div>

          <div className="border rounded-lg p-4 bg-muted/30 space-y-4">
            <div className="text-sm font-medium text-center">
              {phase === 'before' && 'Column 2 has two 5s revealed and one hidden. What if the hidden card is also a 5?'}
              {phase === 'matched' && 'All three cards match! The column will be cleared.'}
              {phase === 'cleared' && 'Column cleared! Those cards no longer count toward your score.'}
            </div>

            <div className="flex flex-col items-center gap-3">
              <div
                className="grid gap-1.5"
                style={{ gridTemplateColumns: `repeat(${numCols}, 1fr)` }}
              >
                {Array.from({ length: numRows }, (_, r) =>
                  Array.from({ length: numCols }, (_, c) => {
                    const idx = c * numRows + r;
                    const slot = board[idx];
                    const isHighlighted = phase === 'matched' && (idx === 3 || idx === 4 || idx === 5);
                    return (
                      <SkyjoCard
                        key={idx}
                        slot={slot}
                        size="md"
                        highlight={isHighlighted}
                      />
                    );
                  })
                ).flat()}
              </div>

              <div className="flex gap-2">
                {stepLabel && (
                  <Button variant="outline" size="sm" onClick={handleStep}>
                    {stepLabel}
                  </Button>
                )}
                {phase === 'cleared' && (
                  <Button variant="outline" size="sm" onClick={handleReset}>
                    Reset Demo
                  </Button>
                )}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </section>
  );
}

// --- Section: Going Out & Penalties ---

function GoingOutSection() {
  return (
    <section id="going-out" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Going Out & Penalties</h2>
      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="text-sm leading-relaxed space-y-2">
            <p>
              When a player <strong>reveals all</strong> their cards, the round enters its final
              phase. Every other player gets <strong>one more turn</strong>, then the round ends
              and all remaining hidden cards are revealed.
            </p>
            <p>
              The player who went out must have the <strong>solo lowest score</strong> for the
              round. If they don't (including ties), their <strong>positive score is doubled</strong>{' '}
              as a penalty. Scores of 0 or below are never penalized.
            </p>
          </div>

          <div className="border rounded-lg p-4 bg-muted/30">
            <h4 className="text-sm font-semibold mb-3">Penalty Examples</h4>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b">
                    <th className="text-left py-2 pr-4">Scenario</th>
                    <th className="text-center py-2 px-2">Goer's Score</th>
                    <th className="text-center py-2 px-2">Others' Lowest</th>
                    <th className="text-center py-2 px-2">Penalty?</th>
                    <th className="text-center py-2 px-2">Final Score</th>
                  </tr>
                </thead>
                <tbody className="text-muted-foreground">
                  <tr className="border-b">
                    <td className="py-2 pr-4">Solo lowest</td>
                    <td className="text-center py-2 px-2">8</td>
                    <td className="text-center py-2 px-2">15</td>
                    <td className="text-center py-2 px-2 text-green-600">No</td>
                    <td className="text-center py-2 px-2 font-medium">8</td>
                  </tr>
                  <tr className="border-b">
                    <td className="py-2 pr-4">Tied for lowest</td>
                    <td className="text-center py-2 px-2">10</td>
                    <td className="text-center py-2 px-2">10</td>
                    <td className="text-center py-2 px-2 text-red-600">Yes (tie)</td>
                    <td className="text-center py-2 px-2 font-medium">20</td>
                  </tr>
                  <tr className="border-b">
                    <td className="py-2 pr-4">Not lowest</td>
                    <td className="text-center py-2 px-2">12</td>
                    <td className="text-center py-2 px-2">8</td>
                    <td className="text-center py-2 px-2 text-red-600">Yes</td>
                    <td className="text-center py-2 px-2 font-medium">24</td>
                  </tr>
                  <tr>
                    <td className="py-2 pr-4">Negative score</td>
                    <td className="text-center py-2 px-2">-3</td>
                    <td className="text-center py-2 px-2">-5</td>
                    <td className="text-center py-2 px-2 text-green-600">No (exempt)</td>
                    <td className="text-center py-2 px-2 font-medium">-3</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </CardContent>
      </Card>
    </section>
  );
}

// --- Section: Scoring ---

function ScoringSection() {
  const [sampleHistory, setSampleHistory] = useState<GameHistory | null>(null);
  const [loading, setLoading] = useState(false);
  const loadedRef = useRef(false);

  const loadSampleGame = useCallback(async () => {
    if (loadedRef.current) return;
    loadedRef.current = true;
    setLoading(true);
    try {
      const mod = await import('../../pkg/skyjo_wasm.js');
      const result = JSON.parse(
        mod.simulate_one_with_history(
          JSON.stringify({ seed: 42, strategies: ['Greedy', 'Greedy'] })
        )
      );
      setSampleHistory(result.history);
    } catch {
      // WASM not available — show text-only fallback
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSampleGame();
  }, [loadSampleGame]);

  return (
    <section id="scoring" className="scroll-mt-20">
      <h2 className="text-2xl font-bold mb-4">Scoring</h2>
      <Card>
        <CardContent className="pt-6 space-y-4">
          <div className="text-sm leading-relaxed space-y-2">
            <p>
              At the end of each round, every player's score is the <strong>sum of all their
              remaining cards</strong>. Cleared columns count as 0. Scores accumulate across rounds.
            </p>
            <p>
              The game ends when any player's cumulative score reaches <strong>100 or more</strong>.
              The player with the <strong>lowest cumulative score</strong> wins. Ties are possible.
            </p>
          </div>

          {loading && (
            <div className="text-sm text-muted-foreground animate-pulse text-center py-4">
              Generating sample game...
            </div>
          )}

          {sampleHistory && (
            <div className="border rounded-lg p-4 bg-muted/30">
              <h4 className="text-sm font-semibold mb-2">Sample Game Score Sheet</h4>
              <ScoringSheet history={sampleHistory} onClose={() => {}} />
            </div>
          )}
        </CardContent>
      </Card>
    </section>
  );
}

// --- Main Rules Route ---

export default function RulesRoute() {
  useDocumentTitle('Rules');
  return (
    <>
      <h1 className="text-3xl font-bold mb-6">How to Play Skyjo</h1>
      <div className="flex gap-8">
        <TableOfContents />
        <div className="flex-1 space-y-8 min-w-0">
          <OverviewSection />
          <CardsSection />
          <SetupSection />
          <TurnFlowSection />
          <ColumnClearSection />
          <GoingOutSection />
          <ScoringSection />
        </div>
      </div>
    </>
  );
}
