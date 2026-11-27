import { cx, reveal, useId, useLocal } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { diffDocuments, type BlockDiffEntry, type DiffMode, type DocumentDiff } from '../schema/diff';
import { plainText } from '../schema/helpers';
import type { JinshuBlock, JinshuDocument } from '../schema/types';
import { renderBlock, type BlockRenderContext } from '../internal/Blocks';
import { jinshuText } from '../i18n/text';
import { jinshuDiffRecipe as r } from './JinshuDiff.recipe';

export interface JinshuDiffProps {
  /** The earlier document. */
  readonly a?: JinshuDocument;
  /** The later document. */
  readonly b?: JinshuDocument;
  /** A diff computed in Logic with `diffDocuments`, in place of `a` and `b`; for a long document. */
  readonly diff?: DocumentDiff;
  /** `smart` shows words for a light edit and the whole block for a heavy one; `inline` always words; `block` always whole blocks. @default 'smart' */
  readonly mode?: DiffMode;
  readonly className?: string;
  readonly 'data-testid'?: string;
}

type RenderGroup =
  | { readonly kind: 'entry'; readonly entry: BlockDiffEntry; readonly changeIdx: number }
  | { readonly kind: 'collapsed'; readonly key: string; readonly entries: readonly Extract<BlockDiffEntry, { kind: 'unchanged' }>[] };

function groupEntries(entries: readonly BlockDiffEntry[]): RenderGroup[] {
  const groups: RenderGroup[] = [];
  let unchangedRun: Extract<BlockDiffEntry, { kind: 'unchanged' }>[] = [];
  let changeIdx = 0;

  const flushUnchanged = () => {
    if (unchangedRun.length === 0) return;
    groups.push({ kind: 'collapsed', key: `col-${unchangedRun[0].id}`, entries: unchangedRun });
    unchangedRun = [];
  };

  for (const entry of entries) {
    if (entry.kind === 'unchanged') {
      unchangedRun.push(entry);
    } else {
      flushUnchanged();
      groups.push({ kind: 'entry', entry, changeIdx });
      changeIdx += 1;
    }
  }
  flushUnchanged();
  return groups;
}

const READ_CTX: BlockRenderContext = { mode: 'view', isSelectable: false };

function blockTypeLabel(block: JinshuBlock): string {
  return jinshuText('@jinshu.blockType', { type: block.type, level: block.type === 'heading' ? block.level : 0 });
}

/**
 * Reusable read-only diff viewer for draft vs saved, AI suggestions, and version conflicts.
 */
export function JinshuDiff({ a, b, diff: precomputed, mode = 'smart', className, 'data-testid': testId }: JinshuDiffProps) {
  const scope = useId();
  const expandedGroups = useLocal<Record<string, boolean>>('jinshu-diff-expanded', {});
  const activeChange = useLocal('jinshu-diff-active', 0);

  const computed = precomputed ?? (a && b ? diffDocuments(a, b, { mode }) : null);
  if (!computed) {
    return <text className={r.empty.base}>{jinshuText('@jinshu.diff.noData')}</text>;
  }

  const totalChanges = computed.stats.added + computed.stats.removed + computed.stats.modified + computed.stats.moved;
  const groups = groupEntries(computed.entries);

  const jumpTo = (nextIdx: number) => {
    if (totalChanges === 0) return;
    const clamped = ((nextIdx % totalChanges) + totalChanges) % totalChanges;
    activeChange.set(clamped);
    try {
      reveal(`jinshu-${scope}-c-${clamped}`);
    } catch {
      // Ignore if not mounted
    }
  };

  return (
    <view data-testid={testId} className={cx(r.root.base, className)}>
      {/* Summary bar */}
      <view className={r.summary.base}>
        <view className={r.counts.base}>
          <text className={r.total.base}>{jinshuText('@jinshu.diff.total', { count: totalChanges })}</text>
          {computed.stats.added > 0 && <Badge label={jinshuText('@jinshu.diff.addedCount', { count: computed.stats.added })} tone="success" size="sm" />}
          {computed.stats.modified > 0 && <Badge label={jinshuText('@jinshu.diff.modifiedCount', { count: computed.stats.modified })} tone="accent" size="sm" />}
          {computed.stats.removed > 0 && <Badge label={jinshuText('@jinshu.diff.removedCount', { count: computed.stats.removed })} tone="danger" size="sm" />}
          {computed.stats.moved > 0 && <Badge label={jinshuText('@jinshu.diff.movedCount', { count: computed.stats.moved })} tone="info" size="sm" />}
        </view>
        {totalChanges > 0 && (
          <view className={r.stepper.base}>
            <Button label={jinshuText('@jinshu.diff.previous')} size="sm" variant="ghost" onClick={() => jumpTo(activeChange.value - 1)} />
            <text className={r.position.base}>{jinshuText('@jinshu.diff.position', { index: activeChange.value + 1, count: totalChanges })}</text>
            <Button label={jinshuText('@jinshu.diff.next')} size="sm" variant="ghost" onClick={() => jumpTo(activeChange.value + 1)} />
          </view>
        )}
      </view>

      {/* Diff entries */}
      <view className={r.entries.base}>
        {groups.map(group => {
          if (group.kind === 'collapsed') {
            const isOpen = expandedGroups.value[group.key] === true;
            if (!isOpen) {
              return (
                <button
                  key={group.key}
                  onClick={() => expandedGroups.set({ ...expandedGroups.value, [group.key]: true })}
                  className={r.expand.base}
                >
                  <text className={r.expandText.base}>{jinshuText('@jinshu.diff.showUnchanged', { count: group.entries.length })}</text>
                </button>
              );
            }
            return (
              <view key={group.key} className={r.unchanged.base}>
                {group.entries.map((u, idx) => (
                  <view key={u.id} className={r.unchangedBlock.base}>
                    {renderBlock(u.block, idx > 0 ? group.entries[idx - 1].block.type : null, READ_CTX)}
                  </view>
                ))}
              </view>
            );
          }

          const { entry, changeIdx } = group;
          const isFocused = activeChange.value === changeIdx;

          if (entry.kind === 'added') {
            return (
              <view
                key={`add-${entry.id}`}
                id={`jinshu-${scope}-c-${changeIdx}`}
                className={cx(r.change.kind.added, isFocused && r.change.focused)}
              >
                <view className={r.changeHeader.base}>
                  <Badge label={jinshuText('@jinshu.diff.added')} tone="success" size="sm" />
                  <text className={r.blockId.base}>{entry.id}</text>
                </view>
                {renderBlock(entry.block, null, READ_CTX)}
              </view>
            );
          }

          if (entry.kind === 'removed') {
            return (
              <view
                key={`rem-${entry.id}`}
                id={`jinshu-${scope}-c-${changeIdx}`}
                className={cx(r.change.kind.removed, isFocused && r.change.focused)}
              >
                <view className={r.changeHeader.base}>
                  <Badge label={jinshuText('@jinshu.diff.removed')} tone="danger" size="sm" />
                  <text className={r.blockId.base}>{entry.id}</text>
                </view>
                <view className={r.removedBlock.base}>{renderBlock(entry.block, null, READ_CTX)}</view>
              </view>
            );
          }

          if (entry.kind === 'moved') {
            return (
              <view
                key={`mov-${entry.id}`}
                id={`jinshu-${scope}-c-${changeIdx}`}
                className={cx(r.change.kind.moved, isFocused && r.change.focused)}
              >
                <view className={r.changeHeader.base}>
                  <Badge label={jinshuText('@jinshu.diff.moved', { from: entry.fromIndex + 1, to: entry.toIndex + 1 })} tone="info" size="sm" />
                  <text className={r.blockId.base}>{entry.id}</text>
                </view>
                {renderBlock(entry.block, null, READ_CTX)}
              </view>
            );
          }

          // Modified block
          if (entry.kind !== 'modified') return null;
          return (
            <view
              key={`mod-${entry.id}`}
              id={`jinshu-${scope}-c-${changeIdx}`}
              className={cx(r.change.kind.modified, isFocused && r.change.focused)}
            >
              <view className={r.changeHeader.wrapping}>
                <Badge label={jinshuText('@jinshu.diff.modified')} tone="accent" size="sm" />
                <text className={r.blockId.base}>{entry.id}</text>
                {entry.moved && (
                  <Badge label={jinshuText('@jinshu.diff.moved', { from: entry.moved.fromIndex + 1, to: entry.moved.toIndex + 1 })} tone="info" size="sm" />
                )}
                {entry.typeChanged && (
                  <Badge label={`${blockTypeLabel(entry.before)} → ${blockTypeLabel(entry.after)}`} tone="warning" size="sm" />
                )}
                {entry.marksOnly && <Badge label={jinshuText('@jinshu.diff.marksOnly')} tone="info" size="sm" />}
              </view>

              {entry.wordDiff && !entry.fullBlockReplace ? (
                <text className={r.words.base}>
                  {entry.wordDiff.map((seg, idx) =>
                    seg.kind === 'equal' ? (
                      seg.text
                    ) : seg.kind === 'delete' ? (
                      <span key={`d-${idx}`} className={r.words.deleted}>
                        {seg.text}
                      </span>
                    ) : (
                      <span key={`i-${idx}`} className={r.words.inserted}>
                        {seg.text}
                      </span>
                    ),
                  )}
                </text>
              ) : (
                <view className={r.sides.base}>
                  <view className={r.side.before}>
                    <text className={r.sideLabel.before}>{jinshuText('@jinshu.diff.before')}</text>
                    {renderBlock(entry.before, null, READ_CTX)}
                  </view>
                  <view className={r.side.after}>
                    <text className={r.sideLabel.after}>{jinshuText('@jinshu.diff.after')}</text>
                    {renderBlock(entry.after, null, READ_CTX)}
                  </view>
                </view>
              )}

              {entry.attrChanges.length > 0 && (
                <view className={r.attrs.base}>
                  {entry.attrChanges.map(ch => (
                    <text key={ch.key} className={r.attr.base}>
                      {`${ch.key}: ${JSON.stringify(ch.before ?? null)} → ${JSON.stringify(ch.after ?? null)}`}
                    </text>
                  ))}
                </view>
              )}
            </view>
          );
        })}
      </view>
    </view>
  );
}
