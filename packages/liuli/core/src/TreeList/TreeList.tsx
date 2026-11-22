import { cx, useId, useLocal, type Child } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { treeListRecipe as r } from './TreeList.recipe';
import { dataOf } from '../internal/data';

export interface TreeNode {
  /** Identifies the node: what `value` and `onChange` carry, and `<testid>-<id>` on its row. Unique in the tree. */
  id: string;
  label: string;
  /** An icon before the label, by name or as a definition. */
  icon?: IconRef;
  /** A second line under the label: a path, a summary. Read as the row's description. */
  description?: string;
  /** Quiet text at the end of the row: a count, a size. Read after the label. */
  endText?: string;
  /** At the very end of the row, after `endText`: a Badge, or an IconButton with its own action and name. */
  endContent?: Child;
  /** The nodes inside; a node with children is a branch that opens and closes. */
  children?: readonly TreeNode[];
  /** @default false */
  isDisabled?: boolean;
}

export interface TreeListProps {
  /** Names the tree for assistive tech: "Folders". */
  label: string;
  items: readonly TreeNode[];
  /** The selected node's id, or null. */
  value: string | null;
  /** Runs with the id of the row that was pressed; a branch is selected and toggled by the same press. */
  onChange: (id: string) => void;
  /** The ids of the open branches, to hold them outside; pass `onOpenBranchesChange` with it. */
  openBranches?: readonly string[];
  /** Runs with the open branches' ids after a press opens or closes one. */
  onOpenBranchesChange?: (ids: readonly string[]) => void;
  /** The branches open at first, when `openBranches` is not given. @default [] */
  defaultOpenBranches?: readonly string[];
  /** Hairlines down from each open branch's chevron beside its children, so deep levels read at a glance. @default false */
  hasLineGuides?: boolean;
  className?: string;
  'data-testid'?: string;
}

type Level = 1 | 2 | 3 | 4 | 5 | 6;
type Guide = 1 | 2 | 3 | 4 | 5;

/** Things inside things: folders, an outline, a file tree. Branches open in place. */
export function TreeList({
  label, items, value, onChange, openBranches, onOpenBranchesChange, defaultOpenBranches = [], hasLineGuides = false, className, 'data-testid': testId, ...attributes
}: TreeListProps) {
  const id = useId();
  const local = useLocal<readonly string[]>('open', defaultOpenBranches);
  const shown = openBranches ?? local.value;
  const toggle = (id: string) => {
    const next = shown.includes(id) ? shown.filter(each => each !== id) : [...shown, id];
    if (openBranches === undefined) local.set(next);
    onOpenBranchesChange?.(next);
  };
  // `lastAt[n]` says whether the row's ancestor at depth n + 1 (or the row itself) is the last of its siblings.
  const rows = (list: readonly TreeNode[], depth: number, lastAt: readonly boolean[]): JSX.Element[] => list.flatMap((node, index) => {
    const isBranch = node.children !== undefined;
    const isOpen = isBranch && shown.includes(node.id);
    const isSelected = node.id === value;
    const isLast = [...lastAt, index === list.length - 1];
    const hasEnd = node.endContent !== undefined;
    const describedBy = node.description === undefined ? undefined : `${id}-${node.id}-description`;
    // A guide runs down a parent's chevron column while that parent has more below it in this branch.
    const guides = hasLineGuides && depth > 1 && Array.from({ length: depth - 1 }, (_, level) => {
      const isOwn = level === depth - 2;
      if (!isOwn && isLast[level + 1]) return null;
      return <view key={`guide-${level}`} aria-hidden className={cx(r.guide.base, r.guide.level[(Math.min(level, 4) + 1) as Guide], isOwn && isLast[depth - 1] ? r.guide.last : r.guide.through)} />;
    });
    const button = (
      <button
        key={node.id}
        role="treeitem"
        aria-level={depth}
        aria-expanded={isBranch ? isOpen : undefined}
        aria-selected={isSelected}
        aria-label={node.endText === undefined && node.description === undefined ? undefined : node.endText === undefined ? node.label : `${node.label}, ${node.endText}`}
        aria-describedby={describedBy}
        disabled={node.isDisabled}
        onClick={() => { if (isBranch) toggle(node.id); onChange(node.id); }}
        data-testid={testId && `${testId}-${node.id}`}
        className={cx(
          r.row.base,
          r.row.level[Math.min(depth, 6) as Level],
          hasEnd ? r.row.inItem : isSelected ? r.row.selected : r.row.idle,
          node.description !== undefined && r.row.described,
          hasLineGuides && r.row.guided,
        )}
      >
        {guides}
        {isBranch ? <Glyph name="chevron-down" className={cx(r.chevron.base, !isOpen && r.chevron.closed)} /> : <view className={r.spacer.base} />}
        {node.icon !== undefined && <Glyph name={node.icon} className={r.icon.base} />}
        {node.description === undefined
          ? <text className={cx(r.label.base, isSelected && r.label.selected)}>{node.label}</text>
          : (
            <view className={r.words.base}>
              <text className={cx(r.label.stacked, isSelected && r.label.selected)}>{node.label}</text>
              <text id={describedBy} className={r.description.base}>{node.description}</text>
            </view>
          )}
        {node.endText !== undefined && <text aria-hidden className={r.end.base}>{node.endText}</text>}
      </button>
    );
    // The row's own content cannot sit inside its button, so the two share a row that carries the fill.
    const row = hasEnd
      ? (
        <view key={node.id} className={cx(r.item.base, isSelected ? r.item.selected : r.item.idle)}>
          {button}
          <view className={r.endContent.base}>{node.endContent}</view>
        </view>
      )
      : button;
    return isOpen ? [row, ...rows(node.children ?? [], depth + 1, isLast)] : [row];
  });
  return (
    <view role="tree" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      {rows(items, 1, [])}
    </view>
  );
}
