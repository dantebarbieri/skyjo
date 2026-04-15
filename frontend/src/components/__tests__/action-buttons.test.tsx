import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ActionButtons } from '../action-buttons';

function renderButtons(overrides: Partial<Parameters<typeof ActionButtons>[0]> = {}) {
  const defaults = {
    wantsFlip: false,
    onToggleFlip: vi.fn(),
    onUndo: vi.fn(),
    trashEnabled: true,
    undoEnabled: true,
    ...overrides,
  };
  const result = render(<ActionButtons {...defaults} />);
  return { ...result, ...defaults };
}

describe('ActionButtons', () => {
  it('renders Trash and Undo buttons', () => {
    renderButtons();
    const buttons = screen.getAllByRole('button');
    expect(buttons).toHaveLength(2);
  });

  it('Trash button uses variant="default" when wantsFlip is true', () => {
    renderButtons({ wantsFlip: true });
    const buttons = screen.getAllByRole('button');
    const trashButton = buttons[0];
    // variant="default" does not add data-variant="outline"
    expect(trashButton).not.toHaveAttribute('data-variant', 'outline');
  });

  it('Trash button uses variant="outline" when wantsFlip is false', () => {
    renderButtons({ wantsFlip: false });
    const buttons = screen.getAllByRole('button');
    const trashButton = buttons[0];
    expect(trashButton).toHaveAttribute('data-variant', 'outline');
  });

  it('Trash button is disabled when trashEnabled is false', () => {
    renderButtons({ trashEnabled: false });
    const buttons = screen.getAllByRole('button');
    expect(buttons[0]).toBeDisabled();
  });

  it('Undo button is disabled when undoEnabled is false', () => {
    renderButtons({ undoEnabled: false });
    const buttons = screen.getAllByRole('button');
    expect(buttons[1]).toBeDisabled();
  });

  it('calls onToggleFlip when Trash button is clicked', async () => {
    const user = userEvent.setup();
    const { onToggleFlip } = renderButtons({ trashEnabled: true });
    const buttons = screen.getAllByRole('button');
    await user.click(buttons[0]);
    expect(onToggleFlip).toHaveBeenCalledOnce();
  });

  it('calls onUndo when Undo button is clicked', async () => {
    const user = userEvent.setup();
    const { onUndo } = renderButtons({ undoEnabled: true });
    const buttons = screen.getAllByRole('button');
    await user.click(buttons[1]);
    expect(onUndo).toHaveBeenCalledOnce();
  });

  it('both buttons are disabled when both trashEnabled and undoEnabled are false', () => {
    renderButtons({ trashEnabled: false, undoEnabled: false });
    const buttons = screen.getAllByRole('button');
    expect(buttons[0]).toBeDisabled();
    expect(buttons[1]).toBeDisabled();
    // Both should have outline variant when disabled
    expect(buttons[0]).toHaveAttribute('data-variant', 'outline');
    expect(buttons[1]).toHaveAttribute('data-variant', 'outline');
  });
});
