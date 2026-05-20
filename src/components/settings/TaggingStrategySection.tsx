import { CustomSelect } from '../ui/CustomSelect';
import { OverrideControls } from './OverrideControls';

/**
 * Auto-tagging strategy + k-NN tunables.
 *
 * Mirrors the strategy enum on the Rust side (see
 * `crates/atomic-core/src/embedding.rs::TaggingStrategy`):
 *
 *   - knn_then_llm (default): k-NN inheritance over `vec_chunks` runs first
 *     and applies any tag carried by ≥ min_consensus of the K nearest
 *     neighbors at ≥ min_similarity. The LLM extractor then runs and may
 *     add (never remove) further tags. Best for established libraries.
 *   - knn_only: k-NN only; no LLM call. Deterministic and cheap, but won't
 *     surface tags whose corpus is too sparse to clear consensus.
 *   - truncated_full_content: legacy LLM-only path on the document head.
 *   - chunk_assisted: legacy LLM-only with retrieved chunk citations.
 *
 * The k-NN tunables are hidden when the selected strategy doesn't use k-NN
 * — there's no point letting the user nudge dials that the pipeline will
 * not consult.
 */
const STRATEGY_OPTIONS = [
  { value: 'knn_then_llm', label: 'k-NN, then LLM (recommended)' },
  { value: 'knn_only', label: 'k-NN only (deterministic, no LLM)' },
  { value: 'truncated_full_content', label: 'LLM only — truncated full content' },
  { value: 'chunk_assisted', label: 'LLM only — chunk-assisted' },
];

const STRATEGY_DESCRIPTIONS: Record<string, string> = {
  knn_then_llm:
    'Inherit tags from your most similar atoms, then ask the LLM to add anything missing. Reuses canonical tags reliably and falls back to the LLM for genuinely novel notes.',
  knn_only:
    'Inherit tags from your most similar atoms only. Skips the LLM entirely — fastest and fully deterministic, but new topics with no neighbors get no tags.',
  truncated_full_content:
    'Send a truncated version of the atom to the LLM and let it pick tags from your auto-tag categories. Legacy default; can miss canonical tags when many similar ones exist.',
  chunk_assisted:
    'Same as truncated full content, but the LLM also sees retrieved chunk citations from neighboring atoms. More expensive; rarely necessary alongside k-NN.',
};

function strategyUsesKnn(strategy: string): boolean {
  return strategy === 'knn_then_llm' || strategy === 'knn_only';
}

interface NumberFieldProps {
  label: string;
  description: string;
  value: string;
  onChange: (value: string) => void;
  onCommit: (value: string) => void;
  min: number;
  max: number;
  step: number;
  settingKey: string;
}

/**
 * Local number input with autoSave-on-blur semantics, matching the existing
 * `Embedding Dimension` field in SettingsModal.tsx so override and override
 * controls keep working.
 */
function NumberField({
  label,
  description,
  value,
  onChange,
  onCommit,
  min,
  max,
  step,
  settingKey,
}: NumberFieldProps) {
  return (
    <div className="space-y-1">
      <label className="block text-sm font-medium text-[var(--color-text-primary)]">
        {label}
      </label>
      <p className="text-xs text-[var(--color-text-secondary)]">{description}</p>
      <input
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onBlur={() => onCommit(value)}
        className="w-full px-3 py-2 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] placeholder-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)] focus:border-transparent transition-colors duration-150"
      />
      <OverrideControls settingKey={settingKey} />
    </div>
  );
}

interface Props {
  strategy: string;
  onStrategyChange: (value: string) => void;

  knnK: string;
  onKnnKChange: (value: string) => void;
  onKnnKCommit: (value: string) => void;

  knnMinConsensus: string;
  onKnnMinConsensusChange: (value: string) => void;
  onKnnMinConsensusCommit: (value: string) => void;

  knnMinSimilarity: string;
  onKnnMinSimilarityChange: (value: string) => void;
  onKnnMinSimilarityCommit: (value: string) => void;
}

export function TaggingStrategySection({
  strategy,
  onStrategyChange,
  knnK,
  onKnnKChange,
  onKnnKCommit,
  knnMinConsensus,
  onKnnMinConsensusChange,
  onKnnMinConsensusCommit,
  knnMinSimilarity,
  onKnnMinSimilarityChange,
  onKnnMinSimilarityCommit,
}: Props) {
  const description =
    STRATEGY_DESCRIPTIONS[strategy] ??
    'Choose how new atoms get their tags assigned.';
  const showKnnKnobs = strategyUsesKnn(strategy);

  return (
    <div className="space-y-4 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-panel)] p-4">
      <div className="space-y-1">
        <label className="block text-sm font-medium text-[var(--color-text-primary)]">
          Tagging Strategy
        </label>
        <p className="text-xs text-[var(--color-text-secondary)]">{description}</p>
        <CustomSelect
          value={strategy}
          onChange={onStrategyChange}
          options={STRATEGY_OPTIONS}
        />
        <OverrideControls settingKey="tagging_strategy" />
      </div>

      {showKnnKnobs && (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <NumberField
            label="Neighbors (K)"
            description="How many of the most similar atoms vote. Higher = broader, more conservative."
            value={knnK}
            onChange={onKnnKChange}
            onCommit={onKnnKCommit}
            min={1}
            max={50}
            step={1}
            settingKey="knn_tagging_k"
          />
          <NumberField
            label="Min consensus"
            description="A tag is applied only when at least this many neighbors carry it. Must be ≤ K."
            value={knnMinConsensus}
            onChange={onKnnMinConsensusChange}
            onCommit={onKnnMinConsensusCommit}
            min={1}
            max={50}
            step={1}
            settingKey="knn_tagging_min_consensus"
          />
          <NumberField
            label="Min similarity"
            description="Lowest chunk-level similarity that counts as a neighbor. 0.55 is a balanced default."
            value={knnMinSimilarity}
            onChange={onKnnMinSimilarityChange}
            onCommit={onKnnMinSimilarityCommit}
            min={0}
            max={1}
            step={0.05}
            settingKey="knn_tagging_min_similarity"
          />
        </div>
      )}
    </div>
  );
}
