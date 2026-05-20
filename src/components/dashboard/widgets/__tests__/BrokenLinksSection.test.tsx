import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrokenLinksSection } from '../review/BrokenLinksSection';

const invoke = vi.fn();
vi.mock('../../../../lib/transport', () => ({
  getTransport: () => ({ invoke }),
}));

// Suppress toast errors from sonner (not mounted in test environment)
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), {
    error: vi.fn(),
    success: vi.fn(),
  }),
}));

// Suppress our toast wrapper
vi.mock('../../../../stores/toasts', () => ({
  toast: {
    error: vi.fn(),
    info: vi.fn(),
    success: vi.fn(),
  },
}));

const makeData = () => ({
  broken_link_list: [
    {
      atom_id: 'atom-1',
      atom_title: 'First Atom',
      links: [{ raw: '[[Missing Page]]', target: 'Missing Page', kind: 'wikilink' }],
    },
    {
      atom_id: 'atom-2',
      atom_title: 'Second Atom',
      links: [{ raw: '[broken](./gone.md)', target: './gone.md', kind: 'markdown' }],
    },
  ],
});

// ─── Tests that don't need fake timers ───────────────────────────────────────
describe('BrokenLinksSection', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ status: 'ok' });
  });

  afterEach(() => {
    cleanup();
  });

  it('renders atom titles and link raws', () => {
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);
    expect(screen.getByText('First Atom')).toBeTruthy();
    expect(screen.getByText('Second Atom')).toBeTruthy();
    expect(screen.getByText('[[Missing Page]]')).toBeTruthy();
    expect(screen.getByText('[broken](./gone.md)')).toBeTruthy();
  });

  it('Auto-fix (LLM) and Remove buttons are visible', () => {
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);
    const autoFixBtns = screen.getAllByRole('button', { name: /Auto-fix with LLM/i });
    expect(autoFixBtns[0]).toBeTruthy();
    const removeBtns = screen.getAllByRole('button', { name: /Remove link/i });
    expect(removeBtns[0]).toBeTruthy();
  });

  it('dispatches remove_link with correct action and content on Remove link click', async () => {
    const onResolved = vi.fn();
    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={onResolved} />);

    const removeBtns = screen.getAllByRole('button', { name: /Remove link/i });
    await user.click(removeBtns[0]);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'apply_health_item_fix',
        expect.objectContaining({
          check: 'broken_internal_links',
          item_id: 'atom-1',
          action: 'remove_link',
          content: '[[Missing Page]]',
        }),
      ),
    );
  });

  it('dispatches dismiss on Ignore click', async () => {
    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);

    const ignoreBtns = screen.getAllByRole('button', { name: /^Ignore link$/i });
    await user.click(ignoreBtns[0]);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'apply_health_item_fix',
        expect.objectContaining({
          check: 'broken_internal_links',
          item_id: 'atom-1',
          action: 'dismiss',
        }),
      ),
    );
  });

  it('per-row Auto-fix (LLM) calls auto_resolve with link raw', async () => {
    invoke.mockResolvedValue({ outcome: 'relinked', reason: 'Found a match' });
    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);

    const autoFixBtns = screen.getAllByRole('button', { name: /Auto-fix with LLM/i });
    await user.click(autoFixBtns[0]);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'apply_health_item_fix',
        expect.objectContaining({
          check: 'broken_internal_links',
          item_id: 'atom-1',
          action: 'auto_resolve',
          content: '[[Missing Page]]',
        }),
      ),
    );
  });

  it('Auto-fix all button calls health_broken_links_auto_resolve_all', async () => {
    invoke.mockResolvedValue({ checked: 2, relinked: 1, removed: 1, skipped: 0 });
    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);

    const autoFixAllBtn = screen.getByRole('button', { name: /Auto-fix all broken links/i });
    await user.click(autoFixAllBtn);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('health_broken_links_auto_resolve_all', {}),
    );
  });

  it('shows empty state when broken_link_list is empty', () => {
    render(<BrokenLinksSection data={{ broken_link_list: [] }} onResolved={vi.fn()} />);
    expect(screen.getByText(/No broken internal links/)).toBeTruthy();
  });

  it('Link… opens picker with link.target prefilled', async () => {
    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);

    const linkBtns = screen.getAllByRole('button', { name: /Link to atom/i });
    await user.click(linkBtns[0]);

    const input = screen.getByPlaceholderText('Search atoms…') as HTMLInputElement;
    expect(input).toBeTruthy();
    expect(input.value).toBe('Missing Page');
  });
  it('collapses duplicate link raws within one atom into a single row with ×N badge', () => {
    const dupData = {
      broken_link_list: [
        {
          atom_id: 'atom-dup',
          atom_title: 'Repeats',
          links: [
            { raw: '[[Glossary]]', target: 'Glossary', kind: 'wikilink' },
            { raw: '[[Glossary]]', target: 'Glossary', kind: 'wikilink' },
            { raw: '[[Glossary]]', target: 'Glossary', kind: 'wikilink' },
          ],
        },
      ],
    };
    render(<BrokenLinksSection data={dupData} onResolved={vi.fn()} />);
    // Only one LinkRow renders even though the atom has 3 identical raws
    expect(screen.getAllByText('[[Glossary]]').length).toBe(1);
    // Badge surfaces the occurrence count
    expect(screen.getByText('×3')).toBeTruthy();
    // Only one Remove button — backend replace-all will handle all 3 occurrences
    const removeBtns = screen.getAllByRole('button', { name: /Remove link/i });
    expect(removeBtns.length).toBe(1);
  });

  it('By target: groups links across atoms by normalized destination', async () => {
    const user = userEvent.setup();
    const kbData = {
      broken_link_list: [
        {
          atom_id: 'atom-a',
          atom_title: 'Atom A',
          links: [{ raw: '../docs/glossary.md', target: '../docs/glossary.md', kind: 'markdown' }],
        },
        {
          atom_id: 'atom-b',
          atom_title: 'Atom B',
          links: [{ raw: './glossary.md', target: './glossary.md', kind: 'markdown' }],
        },
        {
          atom_id: 'atom-c',
          atom_title: 'Atom C',
          links: [{ raw: '[[Glossary]]', target: 'Glossary', kind: 'wikilink' }],
        },
      ],
    };
    render(<BrokenLinksSection data={kbData} onResolved={vi.fn()} />);
    const byTarget = screen.getByRole('button', { name: /By target/i });
    await user.click(byTarget);
    // 3 atoms → all share normalized target 'glossary' → one group
    await waitFor(() => screen.getByText(/1 unique broken target/i));
    expect(screen.getByText('Atom A')).toBeTruthy();
    expect(screen.getByText('Atom B')).toBeTruthy();
    expect(screen.getByText('Atom C')).toBeTruthy();
    expect(screen.getByText(/3 atoms/i)).toBeTruthy();
  });

  it('By target: Remove all fans out via health_fix_batch', async () => {
    invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
      if (cmd === 'health_fix_batch') {
        const items = (args as { items: Array<{ item_id: string }> }).items;
        return Promise.resolve({ results: items.map(i => ({ check: 'broken_internal_links', item_id: i.item_id, ok: true })) });
      }
      return Promise.resolve({ status: 'ok' });
    });
    const onResolved = vi.fn();
    const user = userEvent.setup();
    const kbData = {
      broken_link_list: [
        { atom_id: 'a1', atom_title: 'A1', links: [{ raw: '[[X]]', target: 'X', kind: 'wikilink' }] },
        { atom_id: 'a2', atom_title: 'A2', links: [{ raw: '[[X]]', target: 'X', kind: 'wikilink' }] },
      ],
    };
    render(<BrokenLinksSection data={kbData} onResolved={onResolved} />);
    await user.click(screen.getByRole('button', { name: /By target/i }));
    await user.click(screen.getByRole('button', { name: /Remove all/i }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('health_fix_batch', expect.objectContaining({
        items: expect.arrayContaining([
          expect.objectContaining({ check: 'broken_internal_links', item_id: 'a1', action: 'remove_link', content: '[[X]]' }),
          expect.objectContaining({ check: 'broken_internal_links', item_id: 'a2', action: 'remove_link', content: '[[X]]' }),
        ]),
      })),
    );
    // onResolved must fire with (atomId, raw) for every atom whose batch result was ok.
    // `raw` is the exact link text just removed — the modal reducer needs it so
    // it can prune only that (atom, raw) pair and leave unrelated broken links intact.
    await waitFor(() => expect(onResolved).toHaveBeenCalledWith('a1', '[[X]]'));
    await waitFor(() => expect(onResolved).toHaveBeenCalledWith('a2', '[[X]]'));
  });

});

// ─── Tests that need debounce (200 ms) ─────────────────────────────────────
// Use real timers — waitFor (default 1 s) covers the 200 ms debounce fine.
describe('BrokenLinksSection (debounce)', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ status: 'ok' });
  });

  afterEach(() => {
    cleanup();
  });

  it('shows suggestions after typing query', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'health_broken_link_suggest') {
        return Promise.resolve({ suggestions: [{ atom_id: 'atom-99', title: 'Found Atom', source_url: null, score: 0.9 }] });
      }
      return Promise.resolve({ status: 'ok' });
    });

    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);

    const linkBtns = screen.getAllByRole('button', { name: /Link to atom/i });
    await user.click(linkBtns[0]);

    const input = screen.getByPlaceholderText('Search atoms…');
    await user.clear(input);
    await user.type(input, 'Found');

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('health_broken_link_suggest', expect.objectContaining({ q: 'Found', limit: 5 }));
    }, { timeout: 1000 });

    await waitFor(() => {
      expect(screen.getByText('Found Atom')).toBeTruthy();
    });
  });

  it('clicking suggestion dispatches relink with correct args', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'health_broken_link_suggest') {
        return Promise.resolve({ suggestions: [{ atom_id: 'atom-99', title: 'Found Atom', source_url: null, score: 0.9 }] });
      }
      return Promise.resolve({ status: 'ok' });
    });

    const user = userEvent.setup();
    render(<BrokenLinksSection data={makeData()} onResolved={vi.fn()} />);

    const linkBtns = screen.getAllByRole('button', { name: /Link to atom/i });
    await user.click(linkBtns[0]);

    const input = screen.getByPlaceholderText('Search atoms…');
    await user.clear(input);
    await user.type(input, 'Found');

    await waitFor(() => expect(screen.getByText('Found Atom')).toBeTruthy(), { timeout: 1000 });

    await user.click(screen.getByText('Found Atom'));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'apply_health_item_fix',
        expect.objectContaining({
          check: 'broken_internal_links',
          item_id: 'atom-1',
          action: 'relink',
          content: '[[Missing Page]]',
          into_tag_id: 'atom-99',
        }),
      ),
    );
  });
});
