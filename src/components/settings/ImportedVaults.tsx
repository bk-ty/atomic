/**
 * Imported Vaults registry — settings panel.
 *
 * Surfaces every folder source that's been imported (one row per `vaults`
 * registry entry) so the user can re-sync with one click instead of
 * re-picking the folder every time. Stale-path rows (folder moved/deleted
 * since import) are flagged inline and offer a re-bind action.
 */
import { useCallback, useEffect, useState } from 'react';
import { Loader2, FolderSync, FolderX, Trash2, FolderInput } from 'lucide-react';
import {
  listVaults,
  syncVault,
  rebindVault,
  deleteVault,
  type Vault,
  type ImportResult,
} from '../../lib/api';
import { pickDirectory } from '../../lib/platform';
import { useAtomsStore } from '../../stores/atoms';
import { useTagsStore } from '../../stores/tags';
import { Button } from '../ui/Button';

function relativeTime(iso: string | null): string {
  if (!iso) return 'never synced';
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return 'unknown';
  const seconds = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return `${Math.floor(months / 12)}y ago`;
}

interface RowState {
  busy: boolean;
  result: ImportResult | null;
  error: string | null;
}

const idle: RowState = { busy: false, result: null, error: null };

export function ImportedVaults() {
  const [vaults, setVaults] = useState<Vault[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [rowState, setRowState] = useState<Record<number, RowState>>({});
  const fetchAtoms = useAtomsStore((s) => s.fetchAtoms);
  const fetchTags = useTagsStore((s) => s.fetchTags);

  const reload = useCallback(async () => {
    try {
      const v = await listVaults();
      setVaults(v);
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const setRow = (id: number, patch: Partial<RowState>) =>
    setRowState((prev) => ({ ...prev, [id]: { ...idle, ...prev[id], ...patch } }));

  const handleSync = async (vault: Vault) => {
    setRow(vault.id, { busy: true, result: null, error: null });
    try {
      const result = await syncVault(vault.id);
      setRow(vault.id, { busy: false, result });
      if (result.imported > 0 || result.updated > 0) {
        await Promise.all([fetchAtoms(), fetchTags()]);
      }
      await reload();
    } catch (e) {
      setRow(vault.id, { busy: false, error: String(e) });
    }
  };

  const handleRebind = async (vault: Vault) => {
    const next = await pickDirectory(`Find folder for "${vault.name}"`);
    if (!next) return;
    setRow(vault.id, { busy: true, result: null, error: null });
    try {
      await rebindVault(vault.id, next);
      await reload();
      setRow(vault.id, { busy: false });
    } catch (e) {
      setRow(vault.id, { busy: false, error: String(e) });
    }
  };

  const handleDelete = async (vault: Vault) => {
    if (
      !confirm(
        `Stop tracking "${vault.name}"?\n\nThis only removes the entry from this list. Imported atoms are not deleted.`,
      )
    ) {
      return;
    }
    try {
      await deleteVault(vault.id);
      await reload();
    } catch (e) {
      setRow(vault.id, { busy: false, error: String(e) });
    }
  };

  if (loadError) {
    return <div className="text-sm text-red-400">Failed to load vaults: {loadError}</div>;
  }

  if (vaults === null) {
    return (
      <div className="text-sm text-[var(--color-text-tertiary)] flex items-center gap-2">
        <Loader2 className="w-4 h-4 animate-spin" strokeWidth={2} /> Loading vaults…
      </div>
    );
  }

  if (vaults.length === 0) {
    return (
      <div className="text-xs text-[var(--color-text-tertiary)]">
        No vaults imported yet. Use "Markdown Folder" below to import one — Atomic will remember
        the folder and offer one-click re-sync here.
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {vaults.map((vault) => {
        const state = rowState[vault.id] ?? idle;
        return (
          <div
            key={vault.id}
            className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-secondary)]/40 p-3 space-y-2"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-[var(--color-text-primary)] truncate">
                    {vault.name}
                  </span>
                  <span className="text-xs text-[var(--color-text-tertiary)] shrink-0">
                    · {vault.atom_count} note{vault.atom_count === 1 ? '' : 's'} · synced{' '}
                    {relativeTime(vault.last_synced_at)}
                  </span>
                </div>
                <div
                  className="mt-0.5 text-[11px] font-mono text-[var(--color-text-tertiary)] truncate"
                  title={vault.path}
                >
                  {vault.path}
                </div>
                {!vault.path_exists && (
                  <div className="mt-1 text-[11px] text-amber-400 flex items-center gap-1">
                    <FolderX className="w-3 h-3" strokeWidth={2} /> Folder no longer exists at this
                    path
                  </div>
                )}
              </div>
              <div className="flex items-center gap-1 shrink-0">
                {vault.path_exists ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => handleSync(vault)}
                    disabled={state.busy}
                    title="Re-import this vault"
                  >
                    {state.busy ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" strokeWidth={2} />
                    ) : (
                      <FolderSync className="w-3.5 h-3.5" strokeWidth={2} />
                    )}
                  </Button>
                ) : (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => handleRebind(vault)}
                    disabled={state.busy}
                    title="Pick the folder's new location"
                  >
                    <FolderInput className="w-3.5 h-3.5" strokeWidth={2} />
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleDelete(vault)}
                  disabled={state.busy}
                  title="Stop tracking (atoms preserved)"
                >
                  <Trash2 className="w-3.5 h-3.5" strokeWidth={2} />
                </Button>
              </div>
            </div>
            {state.result && (
              <div className="text-[11px] text-emerald-400">
                Synced: {state.result.imported} new
                {state.result.updated > 0 ? `, ${state.result.updated} updated` : ''}
                {state.result.skipped > 0 ? `, ${state.result.skipped} unchanged` : ''}
                {state.result.errors > 0 ? `, ${state.result.errors} error(s)` : ''}.
              </div>
            )}
            {state.error && (
              <div className="text-[11px] text-red-400 break-words">{state.error}</div>
            )}
          </div>
        );
      })}
    </div>
  );
}
