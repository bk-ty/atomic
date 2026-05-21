import { memo, MouseEvent, useCallback } from 'react';
import { ChevronRight, MessageCircle, FileText } from 'lucide-react';
import { TagWithCount, useTagsStore } from '../../stores/tags';
import { useUIStore } from '../../stores/ui';

interface TagNodeProps {
  tag: TagWithCount;
  level: number;
  selectedTagId: string | null;
  onSelect: (tagId: string) => void;
  onContextMenu: (e: MouseEvent, tag: TagWithCount) => void;
}

export const TagNode = memo(function TagNode({ tag, level, selectedTagId, onSelect, onContextMenu }: TagNodeProps) {
  const openChatSidebar = useUIStore(s => s.openChatSidebar);
  const isExpanded = useUIStore(s => !!s.expandedTagIds[tag.id]);
  const toggleTagExpanded = useUIStore(s => s.toggleTagExpanded);
  const fetchTagChildren = useTagsStore(s => s.fetchTagChildren);
  const hasChildren = (tag.children && tag.children.length > 0) || tag.children_total > 0;
  const isSelected = selectedTagId === tag.id;

  const handleToggle = useCallback(async (e: MouseEvent) => {
    e.stopPropagation();
    // Flip expanded state immediately so the click feels responsive.
    toggleTagExpanded(tag.id);
    const willExpand = !isExpanded;
    if (willExpand && tag.children_total > tag.children.length) {
      try {
        await fetchTagChildren(tag.id);
      } catch (err) {
        // Fetch failed — collapse back so the user can retry.
        toggleTagExpanded(tag.id);
        throw err;
      }
    }
  }, [isExpanded, tag.children_total, tag.children.length, tag.id, fetchTagChildren, toggleTagExpanded]);

  const handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    onContextMenu(e, tag);
  };

  const openWikiReader = useUIStore(s => s.openWikiReader);

  const handleWikiClick = (e: MouseEvent) => {
    e.stopPropagation();
    openWikiReader(tag.id, tag.name, undefined, { newTab: e.metaKey || e.ctrlKey });
  };

  const handleChatClick = (e: MouseEvent) => {
    e.stopPropagation();
    openChatSidebar(tag.id);
  };

  return (
    <div
      className={`group flex items-center gap-1 px-2 py-1.5 rounded-md cursor-pointer transition-colors ${
        isSelected
          ? 'bg-[var(--color-accent)]/20 text-[var(--color-text-primary)]'
          : 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]'
      }`}
      style={{ paddingLeft: `${8 + level * 16}px` }}
      onClick={(e) => hasChildren ? handleToggle(e) : onSelect(tag.id)}
      onContextMenu={handleContextMenu}
    >
      {hasChildren ? (
        <button
          onClick={handleToggle}
          className="w-4 h-4 flex items-center justify-center text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
        >
          <ChevronRight className={`w-3 h-3 transition-transform ${isExpanded ? 'rotate-90' : ''}`} strokeWidth={2} />
        </button>
      ) : (
        <span className="w-4" />
      )}
      <span className="flex-1 truncate text-sm">{tag.name}</span>
      {/* Chat icon — visible on hover. For parent tags, scopes the
          conversation to the entire subtree (the backend walks descendants
          via recursive CTE in `get_tag_hierarchy`). */}
      <button
        onClick={handleChatClick}
        className="w-5 h-5 flex items-center justify-center opacity-0 group-hover:opacity-100 text-[var(--color-text-secondary)] hover:text-[var(--color-accent-light)] transition-all"
        title={hasChildren ? 'Chat with this tag and its sub-tags' : 'Chat with this tag'}
      >
        <MessageCircle className="w-3.5 h-3.5" strokeWidth={2} />
      </button>
      {/* Wiki icon — visible on hover. Opening the reader for a parent tag
          will synthesize an article over the entire subtree. */}
      <button
        onClick={handleWikiClick}
        className="w-5 h-5 flex items-center justify-center opacity-0 group-hover:opacity-100 text-[var(--color-text-secondary)] hover:text-[var(--color-accent-light)] transition-all"
        title={hasChildren ? 'View wiki for this tag and its sub-tags' : 'View wiki article'}
      >
        <FileText className="w-3.5 h-3.5" strokeWidth={2} />
      </button>
      {/* Direct atom count. For parents this is the denormalized count of
          atoms tagged *directly* with this tag (not the subtree); skip
          rendering when it's 0 to avoid noise on category-only parents. */}
      {(!hasChildren || tag.atom_count > 0) && (
        <span className="text-xs text-[var(--color-text-tertiary)] tabular-nums">{tag.atom_count}</span>
      )}
    </div>
  );
});
