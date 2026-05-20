import { useState, useEffect, useMemo } from 'react';
import { Loader2, Check, Unlink, Layers, List } from 'lucide-react';
import type { ItemStatus } from './types';
import { runReviewAction } from './reviewActions';
import { getTransport } from '../../../../lib/transport';
import { toast } from '../../../../stores/toasts';

export interface BrokenLink {
  raw: string;
  target: string;
  kind: 'wikilink' | 'markdown' | string;
}

export interface BrokenLinkAtom {
  atom_id: string;
  atom_title: string;
  links: BrokenLink[];
}

interface Suggestion {
  atom_id: string;
  title: string;
  source_url: string | null;
  score: number;
}

/**
 * Collapse multiple occurrences of the same `raw` text within one atom into
 * one entry. The backend's `remove_link` / `relink` now replace every
 * occurrence in one pass, so surfacing N identical rows would make the user
 * click N times for no benefit.
 *
 * `count` preserves the occurrence total so the UI can show `×N` and the
 * backend's response (count = occurrences) aligns with user expectation.
 */
interface DedupedLink extends BrokenLink {
  count: number;
}

function dedupeByRaw(links: BrokenLink[]): DedupedLink[] {
  const byRaw = new Map<string, DedupedLink>();
  for (const link of links) {
    const existing = byRaw.get(link.raw);
    if (existing) {
      existing.count += 1;
    } else {
      byRaw.set(link.raw, { ...link, count: 1 });
    }
  }
  return Array.from(byRaw.values());
}

/**
 * Normalize a link target so the same broken destination reached from
 * different relative paths groups as a single target. Examples:
 *   `../processes/glossary.md#anchor` → `glossary`
 *   `./glossary.md`                   → `glossary`
 *   `[[Glossary]]` target             → `glossary`
 *
 * Used to key the by-target view. We lowercase so `Glossary` and `glossary`
 * collapse into one group — the backend's resolver is also case-insensitive.
 */
function normalizeTarget(target: string): string {
  return stripLinkTargetToStem(target).toLowerCase();
}

interface LinkRowProps {
  link: DedupedLink;
  atomId: string;
  onRemoved: () => void;
  onIgnore: () => void;
}

/**
 * Convert a broken-link target into a search-friendly query seed.
 *
 * - Strips any `#fragment` or `?query` suffix (backend source_urls don't
 *   carry anchors, so leaving them in guarantees zero matches).
 * - Drops the directory path and `.md` / `.markdown` / `.mdx` extension.
 * - Replaces hyphens and underscores with spaces so slug-style filenames
 *   (`custom-application-stewardship`) match title text
 *   (`Custom Application Stewardship`).
 *
 * Examples:
 *   `../processes/foo-bar.md#section` → `foo bar`
 *   `glossary.md`                    → `glossary`
 *   `Tyler Arkansas Glossary`        → `Tyler Arkansas Glossary`
 */
function stripLinkTargetToStem(target: string): string {
  if (!target) return '';
  // Strip fragment/query.
  const clean = target.split(/[#?]/)[0];
  // Take last path segment.
  const segs = clean.split('/');
  const base = segs[segs.length - 1] || clean;
  // Drop extension.
  const stem = base.replace(/\.(md|markdown|mdx)$/i, '');
  // De-slug.
  return stem.replace(/[-_]+/g, ' ').trim();
}

function LinkRow({ link, atomId, onRemoved, onIgnore }: LinkRowProps) {
  const [status, setStatus] = useState<ItemStatus>('idle');
  const [picking, setPicking] = useState(false);
  const [query, setQuery] = useState('');
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loadingSuggestions, setLoadingSuggestions] = useState(false);

  useEffect(() => {
    if (!picking || query.trim().length < 2) { setSuggestions([]); return; }
    const t = window.setTimeout(async () => {
      setLoadingSuggestions(true);
      try {
        const resp = await runReviewAction({
          label: 'Search suggestions',
          command: 'health_broken_link_suggest',
          args: { q: query.trim(), limit: 5 },
        }) as { suggestions: Suggestion[] } | undefined;
        if (resp) setSuggestions(resp.suggestions);
      } finally { setLoadingSuggestions(false); }
    }, 200);
    return () => window.clearTimeout(t);
  }, [query, picking]);

  const openPicker = () => {
    const stem = stripLinkTargetToStem(link.target);
    setQuery(stem);
    setPicking(true);
  };

  const removeLink = async () => {
    setStatus('saving');
    const ok = await runReviewAction({
      label: 'Remove link',
      command: 'apply_health_item_fix',
      args: { check: 'broken_internal_links', item_id: atomId, action: 'remove_link', content: link.raw },
    });
    if (ok === undefined) { setStatus('idle'); return; }
    setStatus('done');
    setTimeout(() => onRemoved(), 400);
  };

  const dismiss = async () => {
    setStatus('saving');
    const ok = await runReviewAction({
      label: 'Ignore link',
      command: 'apply_health_item_fix',
      args: { check: 'broken_internal_links', item_id: atomId, action: 'dismiss' },
    });
    if (ok === undefined) { setStatus('idle'); return; }
    setStatus('done');
    setTimeout(() => onIgnore(), 400);
  };

  const autoFixLlm = async () => {
    setStatus('saving');
    type AutoFixResult = { outcome: 'relinked' | 'removed' | 'skipped'; target_atom_id?: string; confidence?: number; reason?: string };
    const result = await runReviewAction({
      label: 'Auto-fix (LLM)',
      command: 'apply_health_item_fix',
      args: { check: 'broken_internal_links', item_id: atomId, action: 'auto_resolve', content: link.raw },
    }) as AutoFixResult | undefined;
    if (result === undefined) { setStatus('idle'); return; }
    if (result.outcome === 'relinked') {
      toast.success('Link relinked', { detail: result.reason });
      setStatus('done');
      setTimeout(() => onRemoved(), 400);
    } else if (result.outcome === 'removed') {
      toast.success('Link removed', { detail: result.reason });
      setStatus('done');
      setTimeout(() => onRemoved(), 400);
    } else {
      toast.info('Skipped', { detail: result.reason ?? 'LLM could not determine a target' });
      setStatus('idle');
    }
  };

  const relinkTo = async (targetId: string) => {
    setStatus('saving');
    const ok = await runReviewAction({
      label: 'Relink',
      command: 'apply_health_item_fix',
      args: { check: 'broken_internal_links', item_id: atomId, action: 'relink', content: link.raw, into_tag_id: targetId },
    });
    if (ok === undefined) { setStatus('idle'); return; }
    setStatus('done');
    setPicking(false);
    setTimeout(() => onRemoved(), 400);
  };

  if (status === 'done') {
    return (
      <div className="flex items-center gap-2 py-2 px-3 text-xs text-gray-500">
        <Check className="w-3.5 h-3.5 text-green-500 shrink-0" />
        Resolved
      </div>
    );
  }

  return (
    <div className="py-2.5 px-3 border-b border-white/5 last:border-b-0">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-2 min-w-0 flex-1">
          <Unlink className="w-3.5 h-3.5 text-yellow-400/70 mt-0.5 shrink-0" />
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 flex-wrap">
              <code
                className="text-xs text-yellow-300/80 bg-[#161616] rounded px-1.5 py-0.5 truncate max-w-[220px]"
                title={link.raw}
              >
                {link.raw}
              </code>
              <span className="text-xs text-gray-600 truncate">→ {link.target}</span>
              <span className={`px-1.5 py-0.5 rounded text-[10px] shrink-0 ${link.kind === 'wikilink' ? 'bg-purple-900/40 text-purple-300' : 'bg-gray-800 text-gray-400'}`}>
                {link.kind}
              </span>
              {link.count > 1 && (
                <span
                  className="px-1.5 py-0.5 rounded text-[10px] shrink-0 bg-yellow-900/40 text-yellow-300"
                  title={`${link.count} occurrences in this atom — a single Remove or Relink will fix them all`}
                >
                  ×{link.count}
                </span>
              )}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-1.5 shrink-0 flex-wrap justify-end">
          <button
            type="button"
            onClick={autoFixLlm}
            disabled={status === 'saving'}
            className="px-2 py-1 rounded text-[11px] text-white bg-purple-600 hover:bg-purple-500 disabled:opacity-40 inline-flex items-center gap-1"
            title="Let the LLM pick the best target or remove the link"
            aria-label="Auto-fix with LLM"
          >
            {status === 'saving' ? <Loader2 className="w-3 h-3 animate-spin" /> : null}
            Auto-fix (LLM)
          </button>
          <button
            type="button"
            onClick={openPicker}
            disabled={status === 'saving'}
            className="px-2 py-1 rounded text-[11px] text-gray-300 bg-[#2a2a2a] border border-white/10 hover:text-gray-100 disabled:opacity-40"
            title="Search for target atom"
            aria-label="Link to atom"
          >
            Link to…
          </button>
          <button
            type="button"
            onClick={removeLink}
            disabled={status === 'saving'}
            className="px-2 py-1 rounded text-[11px] text-gray-400 hover:text-red-300 bg-[#2a2a2a] border border-white/10 disabled:opacity-40"
            title={link.count > 1 ? `Remove this link (${link.count} occurrences)` : 'Remove this link from the atom'}
            aria-label="Remove link"
          >
            Remove
          </button>
          <button
            type="button"
            onClick={dismiss}
            disabled={status === 'saving'}
            className="px-2 py-1 rounded text-[11px] text-gray-500 hover:text-gray-300 disabled:opacity-40"
            title="Ignore this broken link"
            aria-label="Ignore link"
          >
            Ignore
          </button>
        </div>
      </div>
      {picking && (
        <div className="mt-2 ml-5 bg-[#161616] rounded border border-white/5 p-2 space-y-1">
          <input
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder="Search atoms…"
            autoFocus
            className="w-full bg-[#1e1e1e] border border-white/10 rounded px-2 py-1 text-xs text-gray-200 focus:outline-none focus:border-purple-500/60"
          />
          {loadingSuggestions && <p className="text-[10px] text-gray-500">searching…</p>}
          <ul className="space-y-0.5 max-h-48 overflow-y-auto">
            {suggestions.map(s => (
              <li key={s.atom_id}>
                <button
                  type="button"
                  onClick={() => relinkTo(s.atom_id)}
                  className="w-full text-left px-2 py-1 rounded hover:bg-purple-900/20 transition-colors"
                >
                  <p className="text-xs text-gray-200 truncate">{s.title || s.atom_id}</p>
                  {s.source_url && <p className="text-[10px] text-gray-600 truncate">{s.source_url}</p>}
                </button>
              </li>
            ))}
            {!loadingSuggestions && suggestions.length === 0 && query.trim().length >= 2 && (
              <li className="text-[10px] text-gray-500 italic px-2">No matches.</li>
            )}
          </ul>
          <div className="flex justify-end pt-1">
            <button type="button" onClick={() => setPicking(false)} className="px-1.5 py-0.5 rounded text-[11px] text-gray-400 hover:text-gray-200">Cancel</button>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * One row in the by-target view: every (atom, raw) pair that points to the
 * same normalized target, resolved in bulk. Fanning out via
 * `health_fix_batch` means a single KB-wide "Glossary was renamed" problem
 * is one click instead of N.
 */
interface TargetGroupAtom {
  atom_id: string;
  atom_title: string;
  raw: string;        // Canonical raw text per (atom, raw) pair
  kind: string;
  count: number;      // Occurrences within this atom
}

interface TargetGroupProps {
  target: string;              // Human-readable target (display label)
  kind: string;                // 'wikilink' | 'markdown' — for the badge
  entries: TargetGroupAtom[];  // Deduped per atom; count reflects in-atom repeats
  onResolved: (atomId: string, raw?: string) => void;
}

function TargetGroup({ target, kind, entries, onResolved }: TargetGroupProps) {
  const [busy, setBusy] = useState<'remove' | 'relink' | null>(null);
  const [picking, setPicking] = useState(false);
  const [query, setQuery] = useState('');
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loadingSuggestions, setLoadingSuggestions] = useState(false);
  const [resolvedAtoms, setResolvedAtoms] = useState<Set<string>>(new Set());

  const visibleEntries = entries.filter(e => !resolvedAtoms.has(e.atom_id));
  const totalOccurrences = visibleEntries.reduce((s, e) => s + e.count, 0);

  useEffect(() => {
    if (!picking || query.trim().length < 2) { setSuggestions([]); return; }
    const t = window.setTimeout(async () => {
      setLoadingSuggestions(true);
      try {
        const resp = await runReviewAction({
          label: 'Search suggestions',
          command: 'health_broken_link_suggest',
          args: { q: query.trim(), limit: 5 },
        }) as { suggestions: Suggestion[] } | undefined;
        if (resp) setSuggestions(resp.suggestions);
      } finally { setLoadingSuggestions(false); }
    }, 200);
    return () => window.clearTimeout(t);
  }, [query, picking]);

  const bulkRemove = async () => {
    setBusy('remove');
    const items = visibleEntries.map(e => ({
      check: 'broken_internal_links',
      item_id: e.atom_id,
      action: 'remove_link',
      content: e.raw,
    }));
    try {
      const resp = await getTransport().invoke<{ results: Array<{ check: string; item_id: string; ok: boolean; error?: string }> }>(
        'health_fix_batch',
        { items },
      );
      const okIds = new Set(resp.results.filter(r => r.ok).map(r => r.item_id));
      const failed = resp.results.filter(r => !r.ok);
      if (failed.length) {
        toast.error(`${failed.length} of ${items.length} failed`, { detail: failed.map(f => f.error).filter(Boolean).join('; ') || undefined });
      } else {
        toast.success(`Removed ${items.length} link${items.length === 1 ? '' : 's'}`);
      }
      setResolvedAtoms(prev => new Set([...prev, ...okIds]));
      // Forward the exact `raw` we just removed so the modal reducer prunes
      // only that link — an atom with other unrelated broken targets stays
      // in the list.
      items.forEach(it => {
        if (okIds.has(it.item_id)) onResolved(it.item_id, it.content);
      });
    } catch (err) {
      toast.error('Bulk remove failed', { detail: err instanceof Error ? err.message : String(err) });
    } finally {
      setBusy(null);
    }
  };

  const bulkRelinkTo = async (targetAtomId: string) => {
    setBusy('relink');
    const items = visibleEntries.map(e => ({
      check: 'broken_internal_links',
      item_id: e.atom_id,
      action: 'relink',
      content: e.raw,
      into_tag_id: targetAtomId,
    }));
    try {
      const resp = await getTransport().invoke<{ results: Array<{ check: string; item_id: string; ok: boolean; error?: string }> }>(
        'health_fix_batch',
        { items },
      );
      const okIds = new Set(resp.results.filter(r => r.ok).map(r => r.item_id));
      const failed = resp.results.filter(r => !r.ok);
      if (failed.length) {
        toast.error(`${failed.length} of ${items.length} failed`, { detail: failed.map(f => f.error).filter(Boolean).join('; ') || undefined });
      } else {
        toast.success(`Relinked ${items.length} atom${items.length === 1 ? '' : 's'}`);
      }
      setResolvedAtoms(prev => new Set([...prev, ...okIds]));
      items.forEach(it => {
        if (okIds.has(it.item_id)) onResolved(it.item_id, it.content);
      });
      setPicking(false);
    } catch (err) {
      toast.error('Bulk relink failed', { detail: err instanceof Error ? err.message : String(err) });
    } finally {
      setBusy(null);
    }
  };

  if (visibleEntries.length === 0) return null;

  const openPicker = () => {
    setQuery(stripLinkTargetToStem(target));
    setPicking(true);
  };

  return (
    <div className="rounded-md border border-white/5 bg-[#1e1e1e] overflow-hidden">
      <div className="px-3 py-2 bg-[#252525] border-b border-white/5 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-1.5 flex-wrap">
            <Unlink className="w-3.5 h-3.5 text-yellow-400/70 shrink-0" />
            <code
              className="text-xs text-yellow-300/90 bg-[#161616] rounded px-1.5 py-0.5 truncate max-w-[280px]"
              title={target}
            >
              {target || '(empty target)'}
            </code>
            <span className={`px-1.5 py-0.5 rounded text-[10px] shrink-0 ${kind === 'wikilink' ? 'bg-purple-900/40 text-purple-300' : 'bg-gray-800 text-gray-400'}`}>
              {kind}
            </span>
          </div>
          <p className="text-[10px] text-gray-500 mt-1">
            {visibleEntries.length} atom{visibleEntries.length !== 1 ? 's' : ''} ·
            {' '}
            {totalOccurrences} link{totalOccurrences !== 1 ? 's' : ''}
          </p>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          <button
            type="button"
            onClick={openPicker}
            disabled={busy !== null}
            className="px-2 py-1 rounded text-[11px] text-gray-300 bg-[#2a2a2a] border border-white/10 hover:text-gray-100 disabled:opacity-40"
            title={`Pick one target atom and relink every occurrence across all ${visibleEntries.length} atoms`}
          >
            Relink all…
          </button>
          <button
            type="button"
            onClick={bulkRemove}
            disabled={busy !== null}
            className="px-2 py-1 rounded text-[11px] text-gray-400 hover:text-red-300 bg-[#2a2a2a] border border-white/10 disabled:opacity-40 inline-flex items-center gap-1"
            title={`Remove every occurrence of this link from ${visibleEntries.length} atom${visibleEntries.length === 1 ? '' : 's'}`}
          >
            {busy === 'remove' ? <Loader2 className="w-3 h-3 animate-spin" /> : null}
            Remove all
          </button>
        </div>
      </div>
      {picking && (
        <div className="m-2 bg-[#161616] rounded border border-white/5 p-2 space-y-1">
          <input
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder="Search target atom…"
            autoFocus
            className="w-full bg-[#1e1e1e] border border-white/10 rounded px-2 py-1 text-xs text-gray-200 focus:outline-none focus:border-purple-500/60"
          />
          {loadingSuggestions && <p className="text-[10px] text-gray-500">searching…</p>}
          <ul className="space-y-0.5 max-h-48 overflow-y-auto">
            {suggestions.map(s => (
              <li key={s.atom_id}>
                <button
                  type="button"
                  onClick={() => bulkRelinkTo(s.atom_id)}
                  disabled={busy !== null}
                  className="w-full text-left px-2 py-1 rounded hover:bg-purple-900/20 transition-colors disabled:opacity-40"
                >
                  <p className="text-xs text-gray-200 truncate">{s.title || s.atom_id}</p>
                  {s.source_url && <p className="text-[10px] text-gray-600 truncate">{s.source_url}</p>}
                </button>
              </li>
            ))}
            {!loadingSuggestions && suggestions.length === 0 && query.trim().length >= 2 && (
              <li className="text-[10px] text-gray-500 italic px-2">No matches.</li>
            )}
          </ul>
          <div className="flex justify-end pt-1">
            <button type="button" onClick={() => setPicking(false)} className="px-1.5 py-0.5 rounded text-[11px] text-gray-400 hover:text-gray-200">Cancel</button>
          </div>
        </div>
      )}
      <ul className="divide-y divide-white/5">
        {visibleEntries.map(e => (
          <li key={`${e.atom_id}::${e.raw}`} className="px-3 py-1.5 flex items-center justify-between gap-2">
            <p className="text-xs text-gray-300 truncate">{e.atom_title || e.atom_id}</p>
            {e.count > 1 && (
              <span className="px-1.5 py-0.5 rounded text-[10px] shrink-0 bg-yellow-900/40 text-yellow-300" title={`${e.count} occurrences within this atom`}>
                ×{e.count}
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

interface Props {
  data: { broken_link_list: BrokenLinkAtom[] };
  /** `raw` identifies the exact link text just resolved. Omitted when the whole atom is resolved (auto-fix-all, dismiss). */
  onResolved: (atomId: string, raw?: string) => void;
}

type ViewMode = 'atom' | 'target';

export function BrokenLinksSection({ data, onResolved }: Props) {
  const [autoFixAllBusy, setAutoFixAllBusy] = useState(false);
  const [mode, setMode] = useState<ViewMode>('atom');

  const visibleAtoms = data.broken_link_list;

  const handleResolved = (atomId: string, raw?: string) => {
    onResolved(atomId, raw);
  };

  /**
   * Regroup the data by normalized target so a broken destination used
   * across many atoms appears once. Each group keeps the original display
   * target, kind, and one `TargetGroupAtom` per (atom, raw) pair — where
   * `count` is the occurrence total within that atom.
   */
  const targetGroups = useMemo(() => {
    const byTarget = new Map<string, {
      target: string;
      kind: string;
      entries: Map<string, TargetGroupAtom>; // keyed by `${atom_id}::${raw}`
    }>();
    for (const atom of visibleAtoms) {
      for (const link of atom.links) {
        const key = normalizeTarget(link.target);
        const group = byTarget.get(key) ?? {
          target: link.target,
          kind: link.kind,
          entries: new Map(),
        };
        const entryKey = `${atom.atom_id}::${link.raw}`;
        const entry = group.entries.get(entryKey);
        if (entry) {
          entry.count += 1;
        } else {
          group.entries.set(entryKey, {
            atom_id: atom.atom_id,
            atom_title: atom.atom_title,
            raw: link.raw,
            kind: link.kind,
            count: 1,
          });
        }
        byTarget.set(key, group);
      }
    }
    return Array.from(byTarget.values())
      .map(g => ({ target: g.target, kind: g.kind, entries: Array.from(g.entries.values()) }))
      .sort((a, b) => b.entries.length - a.entries.length);
  }, [visibleAtoms]);

  const autoFixAll = async () => {
    setAutoFixAllBusy(true);
    try {
      type BatchResult = { checked: number; relinked: number; removed: number; skipped: number };
      const result = await getTransport().invoke<BatchResult>('health_broken_links_auto_resolve_all', {});
      toast.success(
        `Auto-fix complete: ${result.relinked} relinked, ${result.removed} removed, ${result.skipped} skipped`,
      );
      data.broken_link_list.forEach(a => onResolved(a.atom_id));
    } catch (err) {
      toast.error('Auto-fix all failed', { detail: err instanceof Error ? err.message : String(err) });
    } finally {
      setAutoFixAllBusy(false);
    }
  };

  if (visibleAtoms.length === 0) {
    return <p className="text-xs text-gray-500 italic py-2">No broken internal links found.</p>;
  }

  return (
    <div className="space-y-3">
      {/* Header row: counts + view toggle + auto-fix-all */}
      <div className="flex items-center justify-between gap-2 flex-wrap">
        <div className="flex items-center gap-3">
          <p className="text-xs text-gray-500">
            {mode === 'atom'
              ? <>{visibleAtoms.length} atom{visibleAtoms.length !== 1 ? 's' : ''} with broken links</>
              : <>{targetGroups.length} unique broken target{targetGroups.length !== 1 ? 's' : ''}</>}
          </p>
          <div
            role="group"
            aria-label="Broken link view mode"
            className="inline-flex rounded border border-white/10 overflow-hidden"
          >
            <button
              type="button"
              onClick={() => setMode('atom')}
              aria-pressed={mode === 'atom'}
              className={`px-2 py-1 text-[11px] inline-flex items-center gap-1 transition-colors ${mode === 'atom' ? 'bg-purple-600 text-white' : 'bg-[#2a2a2a] text-gray-300 hover:text-gray-100'}`}
              title="Group rows by atom — fix links per source atom"
            >
              <List className="w-3 h-3" />
              By atom
            </button>
            <button
              type="button"
              onClick={() => setMode('target')}
              aria-pressed={mode === 'target'}
              className={`px-2 py-1 text-[11px] inline-flex items-center gap-1 transition-colors border-l border-white/10 ${mode === 'target' ? 'bg-purple-600 text-white' : 'bg-[#2a2a2a] text-gray-300 hover:text-gray-100'}`}
              title="Group rows by destination — fix a renamed or missing page in one shot"
            >
              <Layers className="w-3 h-3" />
              By target
            </button>
          </div>
        </div>
        <button
          type="button"
          onClick={autoFixAll}
          disabled={autoFixAllBusy}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-purple-600 hover:bg-purple-500 disabled:opacity-40 rounded text-xs text-white font-medium transition-colors"
          aria-label="Auto-fix all broken links with LLM"
        >
          {autoFixAllBusy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Unlink className="w-3.5 h-3.5" />}
          {autoFixAllBusy ? 'Fixing…' : 'Auto-fix all broken links'}
        </button>
      </div>

      {mode === 'atom' ? (
        <div className="space-y-3">
          {visibleAtoms.map(atom => (
            <AtomCard
              key={atom.atom_id}
              atom={atom}
              onResolved={handleResolved}
            />
          ))}
        </div>
      ) : (
        <div className="space-y-3">
          {targetGroups.map(group => (
            <TargetGroup
              key={`${normalizeTarget(group.target)}::${group.kind}`}
              target={group.target}
              kind={group.kind}
              entries={group.entries}
              onResolved={handleResolved}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function AtomCard({
  atom,
  onResolved,
}: {
  atom: BrokenLinkAtom;
  onResolved: (atomId: string, raw?: string) => void;
}) {
  const [removedRaws, setRemovedRaws] = useState<Set<string>>(new Set());
  const deduped = useMemo(() => dedupeByRaw(atom.links), [atom.links]);
  const visibleLinks = deduped.filter(l => !removedRaws.has(l.raw));

  if (visibleLinks.length === 0) return null;

  return (
    <div className="rounded-md border border-white/5 bg-[#1e1e1e] overflow-hidden">
      <div className="px-3 py-2 bg-[#252525] border-b border-white/5 flex items-center justify-between">
        <p className="text-xs font-medium text-gray-200 truncate">{atom.atom_title || atom.atom_id}</p>
        <span className="text-[10px] text-gray-600 shrink-0 ml-2">
          {visibleLinks.length} broken link{visibleLinks.length !== 1 ? 's' : ''}
        </span>
      </div>
      <div>
        {visibleLinks.map(link => (
          <LinkRow
            key={link.raw}
            link={link}
            atomId={atom.atom_id}
            onRemoved={() => {
              setRemovedRaws(prev => new Set(prev).add(link.raw));
              // Pass raw so the modal's reducer prunes just this (atom, raw)
              // pair. Without raw, an atom with two broken targets disappears
              // when one is fixed and then reappears on refetch.
              onResolved(atom.atom_id, link.raw);
            }}
            onIgnore={() => onResolved(atom.atom_id)}
          />
        ))}
      </div>
    </div>
  );
}
