import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

// --- Mock WASM dependencies ---

vi.mock('@/contexts/wasm-context', () => ({
  useWasmContext: () => ({ ready: true }),
}));

vi.mock('@/hooks/use-wasm', () => ({
  getStrategyDescriptions: () => ({
    strategies: [
      {
        name: 'Random',
        complexity: 'Trivial',
        summary: 'Completely random decisions.',
        strengths: ['Unpredictable'],
        weaknesses: ['No optimization'],
        concepts: [],
        phases: [
          { phase: 'InitialFlips', label: 'Initial Flips', logic: { type: 'Simple' as const, text: 'Random' } },
          { phase: 'ChooseDraw', label: 'Draw', logic: { type: 'Simple' as const, text: 'Random' } },
          { phase: 'DeckDrawAction', label: 'Deck', logic: { type: 'Simple' as const, text: 'Random' } },
          { phase: 'DiscardDrawPlacement', label: 'Discard', logic: { type: 'Simple' as const, text: 'Random' } },
        ],
      },
      {
        name: 'Greedy',
        complexity: 'Low',
        summary: 'Locally optimal.',
        strengths: ['Simple'],
        weaknesses: ['Short-sighted'],
        concepts: [],
        phases: [
          { phase: 'InitialFlips', label: 'Initial Flips', logic: { type: 'Simple' as const, text: 'Pick best' } },
          { phase: 'ChooseDraw', label: 'Draw', logic: { type: 'Simple' as const, text: 'Pick best' } },
          { phase: 'DeckDrawAction', label: 'Deck', logic: { type: 'Simple' as const, text: 'Pick best' } },
          { phase: 'DiscardDrawPlacement', label: 'Discard', logic: { type: 'Simple' as const, text: 'Pick best' } },
        ],
      },
    ],
    common_concepts: [],
  }),
}));

vi.mock('@/hooks/use-document-title', () => ({
  useDocumentTitle: vi.fn(),
}));

// Mock auth context for admin route
vi.mock('@/contexts/auth-context', () => ({
  useAuth: () => ({
    user: { id: '1', username: 'admin', display_name: 'Admin', permission: 'admin' },
    isAuthenticated: true,
    isLoading: false,
    backendAvailable: true,
    accessToken: 'test-token',
    refresh: vi.fn(),
  }),
}));

vi.mock('@/lib/api', () => ({
  apiFetch: vi.fn(() => Promise.resolve({ ok: true, json: () => Promise.resolve([]) })),
}));

// Mock neural network viz (heavy component with its own state)
vi.mock('@/components/neural-network-viz', () => ({
  NeuralNetworkViz: () => <div data-testid="neural-network-viz">Neural Network</div>,
}));

import StrategiesRoute from '../strategies';
import RulesRoute from '../rules';
import AdminRoute from '../admin';
import GeneticManageRoute from '../genetic-manage';

function renderWithRouter(ui: React.ReactElement, initialEntries = ['/']) {
  return render(<MemoryRouter initialEntries={initialEntries}>{ui}</MemoryRouter>);
}

describe('Mobile responsiveness regression tests', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe('Strategies page', () => {
    it('uses flex-col on mobile and flex-row on desktop for main layout', () => {
      const { container } = renderWithRouter(<StrategiesRoute />, ['/rules/strategies']);
      // The main layout container should stack vertically on mobile
      const layoutDiv = container.querySelector('.flex.flex-col.lg\\:flex-row');
      expect(layoutDiv).not.toBeNull();
    });

    it('mobile nav has correct full-bleed margin classes', () => {
      const { container } = renderWithRouter(<StrategiesRoute />, ['/rules/strategies']);
      // Mobile nav should use responsive negative margins matching App container padding
      const mobileNav = container.querySelector('nav.lg\\:hidden');
      expect(mobileNav).not.toBeNull();
      expect(mobileNav!.className).toContain('-mx-3');
      expect(mobileNav!.className).toContain('sm:-mx-4');
    });

    it('desktop sidebar is hidden on mobile', () => {
      const { container } = renderWithRouter(<StrategiesRoute />, ['/rules/strategies']);
      const desktopNav = container.querySelector('nav.hidden.lg\\:block');
      expect(desktopNav).not.toBeNull();
    });
  });

  describe('Rules page', () => {
    it('uses flex-col on mobile and flex-row on desktop for main layout', () => {
      const { container } = renderWithRouter(<RulesRoute />);
      const layoutDiv = container.querySelector('.flex.flex-col.lg\\:flex-row');
      expect(layoutDiv).not.toBeNull();
    });

    it('nav uses lg:self-start instead of self-start for proper mobile width', () => {
      const { container } = renderWithRouter(<RulesRoute />);
      const nav = container.querySelector('nav.sticky');
      expect(nav).not.toBeNull();
      // Should have lg:self-start but NOT bare self-start (which prevents stretching on mobile)
      expect(nav!.className).toContain('lg:self-start');
      // Split into individual classes and check none is exactly "self-start"
      const classes = nav!.className.split(/\s+/);
      expect(classes).not.toContain('self-start');
    });
  });

  describe('Admin page', () => {
    it('wraps user table in overflow-x-auto container', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn(() => Promise.resolve({ ok: true, json: () => Promise.resolve([]) })),
      );
      const { container } = renderWithRouter(<AdminRoute />, ['/admin']);
      // The table should be inside an overflow-x-auto wrapper
      const table = container.querySelector('table');
      expect(table).not.toBeNull();
      const scrollWrapper = table!.closest('.overflow-x-auto');
      expect(scrollWrapper).not.toBeNull();
    });

    it('permission select uses responsive width', () => {
      vi.stubGlobal(
        'fetch',
        vi.fn(() => Promise.resolve({ ok: true, json: () => Promise.resolve([]) })),
      );
      const { container } = renderWithRouter(<AdminRoute />, ['/admin']);
      // Find the create-user form's select trigger — it should use w-full sm:w-[140px]
      const form = container.querySelector('form');
      expect(form).not.toBeNull();
      const selectTrigger = form!.querySelector('[class*="w-full"][class*="sm\\:w-"]');
      expect(selectTrigger).not.toBeNull();
    });
  });

  describe('Genetic manage page', () => {
    it('renders without crashing when no model is loaded', () => {
      vi.stubGlobal(
        'fetch',
        vi.fn(() => Promise.resolve({ ok: false, json: () => Promise.resolve(null) })),
      );
      renderWithRouter(<GeneticManageRoute />, ['/rules/strategies/Genetic/manage']);
      // Should render breadcrumb at minimum
      expect(screen.getByText('Manage')).toBeInTheDocument();
    });
  });
});
