import { useEffect } from 'react';
import { X, ExternalLink, AlertTriangle, Loader2 } from 'lucide-react';
import { useWikiStore } from '../../stores/wiki';
import { Button } from '../ui/Button';
import { openExternalUrl } from '../../lib/platform';

interface WikiCitationsDrawerProps {
  tagId: string;
  tagName: string;
}

/**
 * Right-side drawer that lists every citation in the current wiki article,
 * showing the cited atom's title/excerpt plus whether that atom is still
 * tagged under this wiki's tag. Each row offers an "Untag" action that
 * removes the atom's membership in this tag (leaves the atom itself alone).
 */
export function WikiCitationsDrawer({ tagId, tagName }: WikiCitationsDrawerProps) {
  const isOpen = useWikiStore((s) => s.showCitationsDrawer);
  const details = useWikiStore((s) => s.citationDetails);
  const isLoading = useWikiStore((s) => s.isLoadingCitationDetails);
  const close = useWikiStore((s) => s.closeCitationsDrawer);
  const fetchCitationDetails = useWikiStore((s) => s.fetchCitationDetails);
  const untag = useWikiStore((s) => s.untagAtomFromWiki);

  // Keep the drawer's data in sync with the article that opens it. A new tagId
  // or a re-open should refetch even if we already have stale data cached.
  useEffect(() => {
    if (!isOpen) return;
    fetchCitationDetails(tagId);
  }, [isOpen, tagId, fetchCitationDetails]);

  // Close on Escape.
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [isOpen, close]);

  if (!isOpen) return null;

  const staleCount = details.filter((c) => !c.is_still_tagged).length;
  // Hide duplicates (same atom cited under multiple indexes) — the drawer is
  // per-atom, not per-citation, since "untag" is an atom-level action.
  const uniqueByAtom = new Map<string, typeof details[number]>();
  for (const d of details) {
    if (!uniqueByAtom.has(d.atom_id)) uniqueByAtom.set(d.atom_id, d);
  }
  const atoms = Array.from(uniqueByAtom.values());

  return (
    <div className="absolute inset-y-0 right-0 w-96 bg-[var(--color-bg-panel)] border-l border-[var(--color-border)] shadow-2xl flex flex-col z-40">
      <header className="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)]">
        <div>
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">
            Linked atoms
          </h3>
          <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5">
            Cited in <span className="font-medium">{tagName}</span>
          </p>
        </div>
        <button
          onClick={close}
          className="p-1 rounded hover:bg-[var(--color-bg-secondary)] text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] transition-colors"
          aria-label="Close"
        >
          <X className="w-4 h-4" strokeWidth={2} />
        </button>
      </header>

      {staleCount > 0 && (
        <div className="flex items-start gap-2 px-4 py-2 bg-amber-500/10 border-b border-amber-500/20 text-xs text-amber-400">
          <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" strokeWidth={2} />
          <span>
            {staleCount} cited atom{staleCount === 1 ? '' : 's'} no longer tagged under{' '}
            <strong>{tagName}</strong>. Regenerate the article to drop these citations.
          </span>
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {isLoading && atoms.length === 0 ? (
          <div className="flex items-center justify-center p-8 text-[var(--color-text-tertiary)]">
            <Loader2 className="w-4 h-4 animate-spin mr-2" strokeWidth={2} />
            <span className="text-sm">Loading citations…</span>
          </div>
        ) : atoms.length === 0 ? (
          <div className="p-6 text-sm text-[var(--color-text-tertiary)] text-center">
            No citations in this article.
          </div>
        ) : (
          <ul className="divide-y divide-[var(--color-border)]">
            {atoms.map((c) => (
              <li
                key={c.atom_id}
                className={`px-4 py-3 ${
                  !c.is_still_tagged ? 'bg-amber-500/5' : ''
                }`}
              >
                <div className="flex items-start gap-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs font-mono text-[var(--color-text-tertiary)]">
                        [{c.citation_index}]
                      </span>
                      {!c.is_still_tagged && (
                        <span className="text-[10px] uppercase tracking-wide font-semibold text-amber-400">
                          Untagged
                        </span>
                      )}
                    </div>
                    <p className="text-sm text-[var(--color-text-primary)] leading-snug line-clamp-3">
                      {c.atom_title || c.excerpt || '(no preview)'}
                    </p>
                    {c.source_url && (
                      <button
                        type="button"
                        onClick={() => {
                          void openExternalUrl(c.source_url!);
                        }}
                        className="inline-flex items-center gap-1 mt-1 text-xs text-[var(--color-accent-light)] hover:underline"
                      >
                        <ExternalLink className="w-3 h-3" strokeWidth={2} />
                        Source
                      </button>
                    )}
                  </div>
                </div>
                <div className="mt-2 flex justify-end">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => untag(tagId, c.atom_id)}
                    disabled={!c.is_still_tagged}
                  >
                    {c.is_still_tagged ? 'Untag' : 'Already untagged'}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
