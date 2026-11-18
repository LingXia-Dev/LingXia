import { cx, useLocal } from '@lingxia/view';
import type { IconName } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { treeListRecipe as r } from './TreeList.recipe';

export interface TreeNode {
  /** Identifies the node: what `value` and `onChange` carry, and `<testid>-<id>` on its row. Unique in the tree. */
  id: string;
  label: string;
  /** An icon before the label, by name. */
  icon?: IconName;
  /** Quiet text at the end of the row: a count, a size. Read after the label. */
  endText?: string;
  /** The nodes inside; a node with children is a branch that opens and closes. */
  children?: readonly TreeNode[];
  /** @default false */
  isDisabled?: boolean;
}

export interface TreeListProps {
  /** Names the tree for assistive tech: "Folders". */
  label: string;
  nodes: readonly TreeNode[];
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
  className?: string;
  'data-testid'?: string;
}

type Level = 1 | 2 | 3 | 4 | 5 | 6;

/** Things inside things: folders, an outline, a file tree. Branches open in place. */
export function TreeList({ label, nodes, value, onChange, openBranches, onOpenBranchesChange, defaultOpenBranches = [], className, 'data-testid': testId }: TreeListProps) {
  const local = useLocal<readonly string[]>('open', defaultOpenBranches);
  const shown = openBranches ?? local.value;
  const toggle = (id: string) => {
    const next = shown.includes(id) ? shown.filter(each => each !== id) : [...shown, id];
    if (openBranches === undefined) local.set(next);
    onOpenBranchesChange?.(next);
  };
  const rows = (list: readonly TreeNode[], depth: number): JSX.Element[] => list.flatMap(node => {
    const isBranch = node.children !== undefined;
    const isOpen = isBranch && shown.includes(node.id);
    const isSelected = node.id === value;
    const row = (
      <button
        key={node.id}
        role="treeitem"
        aria-level={depth}
        aria-expanded={isBranch ? isOpen : undefined}
        aria-selected={isSelected}
        aria-label={node.endText === undefined ? undefined : `${node.label}, ${node.endText}`}
        disabled={node.isDisabled}
        onClick={() => { if (isBranch) toggle(node.id); onChange(node.id); }}
        data-testid={testId && `${testId}-${node.id}`}
        className={cx(r.row.base, r.row.level[Math.min(depth, 6) as Level], isSelected ? r.row.selected : r.row.idle)}
      >
        {isBranch ? <Glyph name="chevron-down" className={cx(r.chevron.base, !isOpen && r.chevron.closed)} /> : <view className={r.spacer.base} />}
        {node.icon !== undefined && <Glyph name={node.icon} className={r.icon.base} />}
        <text className={cx(r.label.base, isSelected && r.label.selected)}>{node.label}</text>
        {node.endText !== undefined && <text aria-hidden className={r.end.base}>{node.endText}</text>}
      </button>
    );
    return isOpen ? [row, ...rows(node.children ?? [], depth + 1)] : [row];
  });
  return (
    <view role="tree" aria-label={label} data-testid={testId} className={cx(r.root.base, className)}>
      {rows(nodes, 1)}
    </view>
  );
}
