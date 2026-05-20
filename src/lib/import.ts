/**
 * Client-side markdown folder import.
 *
 * Reads files locally via Tauri FS plugin, parses them, and creates atoms/tags
 * via the transport layer (works with both local and remote servers).
 */

import type { DirEntry } from '@tauri-apps/plugin-fs';
import { getTransport } from './transport';
import type { ImportResult } from './api';
import {
  createTagCache,
  resolveTagIds as resolveTagIdsShared,
  type HierarchicalTag,
  type TagCache,
} from './import-tags';

// Lazy-load Tauri FS plugin (only available in desktop app, not web builds)
async function tauriFs() {
  return await import('@tauri-apps/plugin-fs');
}

// ---------- Types ----------

interface ParsedNote {
  title: string;
  content: string;
  sourceUrl: string;
  frontmatterTags: string[];
  folderTags: HierarchicalTag[];
  relativePath: string;
}

export type { HierarchicalTag };

export interface ImportProgress {
  current: number;
  total: number;
  currentFile: string;
  status: 'importing' | 'skipped' | 'error';
}

// ---------- Constants ----------

const DEFAULT_EXCLUDES = ['.obsidian', '.trash', '.git', 'node_modules'];
const MIN_CONTENT_LENGTH = 10;

// ---------- File discovery ----------

async function discoverMarkdownFiles(
  dirPath: string,
  relativeTo: string,
  excludes: string[],
): Promise<{ absolutePath: string; relativePath: string }[]> {
  const results: { absolutePath: string; relativePath: string }[] = [];
  let entries: DirEntry[];
  try {
    const { readDir } = await tauriFs();
    entries = await readDir(dirPath);
  } catch {
    return results;
  }

  for (const entry of entries) {
    const absPath = `${dirPath}/${entry.name}`;
    const relPath = absPath.slice(relativeTo.length + 1);

    if (entry.isDirectory) {
      if (excludes.includes(entry.name)) continue;
      const children = await discoverMarkdownFiles(absPath, relativeTo, excludes);
      results.push(...children);
    } else if (entry.name.endsWith('.md')) {
      results.push({ absolutePath: absPath, relativePath: relPath });
    }
  }

  return results;
}

// ---------- Frontmatter parsing ----------

function parseFrontmatter(content: string): { yaml: Record<string, unknown> | null; body: string } {
  const match = content.match(/^---\s*\n([\s\S]*?)\n---\s*\n?/);
  if (!match) return { yaml: null, body: content };

  const body = content.slice(match[0].length);
  const yamlStr = match[1];

  // Simple YAML key-value parser (handles strings, arrays, lists)
  const yaml: Record<string, unknown> = {};
  let currentKey: string | null = null;
  let currentList: string[] | null = null;

  for (const line of yamlStr.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    // List item (continuation of a key)
    if (trimmed.startsWith('- ') && currentKey) {
      if (!currentList) currentList = [];
      currentList.push(trimmed.slice(2).trim().replace(/^["']|["']$/g, ''));
      yaml[currentKey] = currentList;
      continue;
    }

    // Flush previous list
    currentList = null;

    const colonIdx = trimmed.indexOf(':');
    if (colonIdx === -1) continue;

    currentKey = trimmed.slice(0, colonIdx).trim();
    let value = trimmed.slice(colonIdx + 1).trim();

    if (!value) {
      // Value will come as list items
      continue;
    }

    // Inline array: [tag1, tag2]
    if (value.startsWith('[') && value.endsWith(']')) {
      yaml[currentKey] = value
        .slice(1, -1)
        .split(',')
        .map((s) => s.trim().replace(/^["']|["']$/g, ''))
        .filter(Boolean);
      continue;
    }

    // Strip quotes
    value = value.replace(/^["']|["']$/g, '');
    yaml[currentKey] = value;
  }

  return { yaml, body };
}

function extractFrontmatterTags(yaml: Record<string, unknown> | null): string[] {
  if (!yaml?.tags) return [];
  const raw = yaml.tags;

  if (Array.isArray(raw)) {
    return raw.map(String).filter(Boolean);
  }
  if (typeof raw === 'string') {
    return raw
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
  }
  return [];
}

// ---------- Tag extraction ----------

function extractFolderTags(relativePath: string): HierarchicalTag[] {
  const parts = relativePath.split('/');
  // Remove filename
  parts.pop();
  if (parts.length === 0) return [];

  return parts.map((name, i) => ({
    name,
    parentPath: parts.slice(0, i),
  }));
}

// ---------- Note parsing ----------

function parseNote(
  content: string,
  relativePath: string,
  vaultName: string,
): ParsedNote | null {
  const { yaml, body } = parseFrontmatter(content);

  // Extract title: frontmatter > first h1 > filename
  let title = yaml?.title as string | undefined;
  if (!title) {
    const h1Match = body.match(/^#\s+(.+)$/m);
    title = h1Match?.[1];
  }
  if (!title) {
    const filename = relativePath.split('/').pop() ?? '';
    title = filename.replace(/\.md$/, '');
  }

  // Build content with title prepended if body doesn't start with h1
  let finalContent = body.trimStart();
  if (!finalContent.startsWith('# ')) {
    finalContent = `# ${title}\n\n${finalContent}`;
  }

  if (finalContent.length < MIN_CONTENT_LENGTH) return null;

  // Generate source URL matching the Rust format. The Rust importer strips
  // the trailing `.md` extension before encoding (see
  // `crates/atomic-core/src/import/obsidian.rs::generate_source_url`); we
  // MUST match it byte-for-byte or `sync_vault` won't find existing atoms
  // imported via this path.
  const pathNoExt = relativePath.replace(/\.md$/i, '');
  const encodedVault = encodeURIComponent(vaultName);
  const encodedPath = pathNoExt
    .split('/')
    .map(encodeURIComponent)
    .join('/');
  const sourceUrl = `obsidian://${encodedVault}/${encodedPath}`;

  return {
    title,
    content: finalContent,
    sourceUrl,
    frontmatterTags: extractFrontmatterTags(yaml),
    folderTags: extractFolderTags(relativePath),
    relativePath,
  };
}

// ---------- Main import function ----------

interface BulkCreateResult {
  atoms: unknown[];
  count: number;
  skipped: number;
}

const BATCH_SIZE = 50;

export interface ImportOptions {
  importTags?: boolean;
  onProgress?: (progress: ImportProgress) => void;
}

export async function importMarkdownFolder(
  folderPath: string,
  options?: ImportOptions,
): Promise<ImportResult> {
  const { importTags = true, onProgress } = options ?? {};
  // Derive vault name from folder path
  const vaultName = folderPath.split('/').pop() ?? 'vault';

  // Load Tauri FS once
  const { readTextFile } = await tauriFs();

  // Discover all .md files
  const files = await discoverMarkdownFiles(folderPath, folderPath, DEFAULT_EXCLUDES);
  const total = files.length;

  let imported = 0;
  let skipped = 0;
  let errors = 0;
  let tagsLinked = 0;

  const tagCache: TagCache = createTagCache();

  // Parse all notes and resolve tags upfront
  const prepared: { note: ParsedNote; tagIds: string[] }[] = [];
  for (let i = 0; i < files.length; i++) {
    const file = files[i];
    onProgress?.({ current: i + 1, total, currentFile: file.relativePath, status: 'importing' });

    try {
      const content = await readTextFile(file.absolutePath);
      const note = parseNote(content, file.relativePath, vaultName);
      if (!note) {
        skipped++;
        continue;
      }

      let tagIds: string[] = [];
      if (importTags) {
        tagIds = await resolveTagIdsShared(note.folderTags, note.frontmatterTags, tagCache);
        tagsLinked += tagIds.length;
      }

      prepared.push({ note, tagIds });
    } catch (e) {
      errors++;
      console.error(`Import: failed to read ${file.relativePath}:`, e);
    }
  }


  // Register the vault in the imported-vaults registry so the user can
  // re-sync from Settings without re-picking the folder. We register
  // *before* the bulk loop (rather than after success) so users who hit a
  // "0 imported, N skipped" pass — the same path that surfaced this bug —
  // still get a registry row they can manage.
  try {
    const { registerVault } = await import('./api');
    await registerVault(vaultName, folderPath, 'obsidian');
  } catch (e) {
    // Non-fatal: the import itself should not fail because the registry
    // upsert hit a transient error. Just log so we surface it during
    // development.
    console.warn('Import: failed to register vault in registry', e);
  }
  // Send in batches via bulk create — server handles dedup
  for (let i = 0; i < prepared.length; i += BATCH_SIZE) {
    const batch = prepared.slice(i, i + BATCH_SIZE);
    const lastFile = batch[batch.length - 1].note.relativePath;
    onProgress?.({
      current: Math.min(i + BATCH_SIZE, prepared.length),
      total: prepared.length,
      currentFile: lastFile,
      status: 'importing',
    });

    try {
      const result = await getTransport().invoke<BulkCreateResult>('bulk_create_atoms', {
        atoms: batch.map(({ note, tagIds }) => ({
          content: note.content,
          sourceUrl: note.sourceUrl,
          skipIfSourceExists: true,
          tagIds,
        })),
      });
      imported += result.count;
      skipped += result.skipped;
    } catch (e) {
      errors += batch.length;
      console.error(`Import: bulk create failed for batch starting at ${i}:`, e);
    }
  }

  return {
    imported,
    updated: 0,
    skipped,
    errors,
    tags_created: tagCache.size,
    tags_linked: tagsLinked,
  };
}
