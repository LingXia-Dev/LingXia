import type { BlockDiffEntry } from '../schema/diff';

export const jinshuDiffParts = {
  root: 'view',
  summary: 'view',
  counts: 'view',
  total: 'text',
  stepper: 'view',
  position: 'text',
  entries: 'view',
  expand: 'button',
  expandText: 'text',
  unchanged: 'view',
  unchangedBlock: 'view',
  change: 'view',
  changeHeader: 'view',
  blockId: 'text',
  removedBlock: 'view',
  words: 'text',
  sides: 'view',
  side: 'view',
  sideLabel: 'text',
  attrs: 'view',
  attr: 'text',
  empty: 'text',
} as const;

/**
 * A summary bar over the changes; each change is a card tinted by its kind, unchanged runs fold
 * into one dashed row, and a modified block shows its words struck and inserted inline.
 */
export const jinshuDiffRecipe = {
  root: {
    base: 'w-full gap-3',
  },
  summary: {
    base: 'flex-row flex-wrap items-center justify-between gap-2 rounded-container border border-line bg-surface px-3.5 py-2.5',
  },
  counts: {
    base: 'flex-row flex-wrap items-center gap-2',
  },
  total: {
    base: 'text-footnote font-medium text-text',
  },
  stepper: {
    base: 'flex-row items-center gap-1.5',
  },
  position: {
    base: 'text-caption text-muted',
  },
  entries: {
    base: 'gap-2.5',
  },
  expand: {
    base: 'rounded-control border border-dashed border-line bg-surface-sunken px-3 py-1.5 items-center hover:bg-text/4',
  },
  expandText: {
    base: 'text-caption text-muted',
  },
  unchanged: {
    base: 'gap-2 text-muted',
  },
  unchangedBlock: {
    base: 'px-3',
  },
  change: {
    kind: {
      added: 'rounded-container border border-success/30 bg-success-soft/60 p-3 gap-1.5',
      removed: 'rounded-container border border-danger/30 bg-danger-soft/60 p-3 gap-1.5',
      moved: 'rounded-container border-s-4 border-info bg-info-soft/40 p-3 gap-1.5',
      modified: 'rounded-container border border-line bg-surface p-3 gap-2',
    } satisfies Record<Exclude<BlockDiffEntry['kind'], 'unchanged'>, string>,
    focused: 'ring-2 ring-accent',
  },
  changeHeader: {
    base: 'flex-row items-center gap-2',
    wrapping: 'flex-row flex-wrap items-center gap-2',
  },
  blockId: {
    base: 'text-caption font-mono text-muted',
  },
  removedBlock: {
    base: 'text-muted',
  },
  words: {
    base: 'text-body leading-relaxed text-text',
    deleted: 'bg-danger-soft text-danger line-through',
    inserted: 'bg-success-soft text-success',
  },
  sides: {
    base: 'gap-2',
  },
  side: {
    before: 'rounded-control bg-danger-soft/50 p-2.5 gap-1',
    after: 'rounded-control bg-success-soft/50 p-2.5 gap-1',
  },
  sideLabel: {
    before: 'text-caption font-medium text-danger',
    after: 'text-caption font-medium text-success',
  },
  attrs: {
    base: 'gap-1 pt-1 border-t border-line',
  },
  attr: {
    base: 'text-caption font-mono text-muted',
  },
  empty: {
    base: 'text-footnote text-muted',
  },
} as const;
