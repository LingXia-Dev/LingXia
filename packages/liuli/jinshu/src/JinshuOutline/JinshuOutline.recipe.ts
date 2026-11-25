import type { OutlineSectionStatus } from './JinshuOutline';

export const jinshuOutlineParts = {
  root: 'view',
  title: 'text',
  item: 'button',
  itemText: 'text',
  status: 'text',
} as const;

/** A quiet list of the document's headings, indented by level; the one being read is tinted. */
export const jinshuOutlineRecipe = {
  root: {
    base: 'gap-1',
  },
  title: {
    base: 'text-caption font-medium text-muted px-2 pb-1',
  },
  item: {
    base: 'flex-row items-center justify-between gap-2 rounded-control py-1.5 pe-2.5 text-start transition-colors duration-fast',
    level: { 1: 'ps-2', 2: 'ps-4', 3: 'ps-6', 4: 'ps-8' },
    active: 'bg-accent-soft text-accent',
    idle: 'hover:bg-text/4 text-text',
  },
  itemText: {
    base: 'text-footnote grow min-w-0 line-clamp-1 truncate',
    active: 'font-medium text-accent',
    top: 'font-medium text-text',
    nested: 'text-muted',
  },
  status: {
    base: 'text-caption shrink-0',
    tone: {
      done: 'text-success font-semibold',
      current: 'text-accent',
      locked: 'text-muted',
    } satisfies Record<OutlineSectionStatus, string>,
  },
} as const;
