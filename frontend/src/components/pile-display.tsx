import { PileCard } from './skyjo-card';
import type { ReplayState } from '@/lib/replay-engine';

interface PileDisplayProps {
  state: ReplayState;
  showDeckTop?: boolean;
}

export default function PileDisplay({ state, showDeckTop = true }: PileDisplayProps) {
  const discardSize = state.discardPiles[0].length;
  const discardTop = discardSize > 0 ? state.discardPiles[0][discardSize - 1] : null;

  let deckValue: number | null = null;
  let deckHint: string | undefined;

  if (state.deck.length > 0) {
    deckValue = showDeckTop ? state.deck[state.deck.length - 1] : null;
    deckHint = showDeckTop ? 'hidden from players' : undefined;
  } else if (state.deckSize > 0) {
    deckValue = null;
    deckHint = 'shuffled';
  }

  return (
    <div className="flex gap-6 items-start justify-center">
      <PileCard
        value={deckValue}
        label="Deck"
        count={state.deckSize}
        hint={deckHint}
        size="md"
      />
      <PileCard
        value={discardTop}
        label="Discard"
        count={discardSize}
        size="md"
      />
    </div>
  );
}
