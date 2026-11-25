import type { Layer } from '../internal/layer';
import type { TableAlign, TableDensity } from './Table';

export const tableParts = {
  root: 'view',
  header: 'view',
  heading: 'text',
  sortHeading: 'button',
  sortMark: 'view',
  sortBar: 'view',
  body: 'view',
  rowgroup: 'view',
  row: 'view',
  cell: 'view',
  text: 'text',
  custom: 'view',
  empty: 'text',
  block: 'view',
  lead: 'view',
  leadCell: 'view',
  figure: 'view',
  pairs: 'view',
  pair: 'view',
  pairHeading: 'text',
  pairValue: 'text',
  measure: 'view',
  sample: 'text',
} as const;

/**
 * A quiet table: headings in muted footnote type over a hairline, rows divided by fainter
 * ones, no stripes. Each row is a grid of the same tracks. A pressable row lands on a `button`.
 */
export const tableRecipe = {
  root: {
    base: 'self-stretch overflow-hidden rounded-container border border-line',
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
  },
  header: {
    base: 'grid items-center gap-x-3 px-4 border-b border-line self-stretch',
    density: { comfortable: 'min-h-control-md', compact: 'min-h-control-sm' } satisfies Record<TableDensity, string>,
    // The rows pass under a heading that stays, so it carries the table's surface.
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
    sticky: 'sticky top-0 z-10',
  },
  heading: {
    base: 'text-footnote font-medium text-muted',
    align: { start: 'text-start', end: 'text-end' } satisfies Record<TableAlign, string>,
    /** The column the rows are ordered by reads in the text colour. */
    sorted: 'text-text',
  },
  /** A heading that orders the rows: its words and a mark, the whole heading height pressable, padded into the row's inset. */
  sortHeading: {
    base: 'flex-row items-center gap-1 self-stretch -mx-1 px-1 rounded-control hover:bg-text/4 active:bg-text/7 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
    align: { start: 'justify-start', end: 'justify-end' } satisfies Record<TableAlign, string>,
  },
  sortMark: {
    base: 'size-3.5 shrink-0',
    on: 'text-text',
    off: 'text-faint',
  },
  /** Stacked: the sort menu's trigger and the order's button at the top of the records, over the hairline the heading would draw. */
  sortBar: {
    base: 'flex-row items-center gap-1 px-4 py-1 border-b border-line self-stretch',
  },
  body: {
    base: 'self-stretch divide-y divide-line',
  },
  rowgroup: {
    base: 'self-stretch',
  },
  row: {
    base: 'grid items-center gap-x-3 px-4 self-stretch text-start',
    density: { comfortable: 'min-h-[44px] py-2', compact: 'min-h-[36px] py-1.5' } satisfies Record<TableDensity, string>,
    pressable: 'transition-colors duration-fast ease-standard',
    idle: 'hover:bg-text/4 active:bg-text/7 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
    selected: 'bg-accent-soft hover:bg-accent/16 active:bg-accent/20 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
  },
  cell: {
    // The text takes the cell's width, so a long value wraps inside its column.
    base: 'min-w-0 items-stretch',
  },
  text: {
    base: 'text-body text-text wrap-anywhere',
    lead: 'font-medium',
    align: { start: 'text-start', end: 'text-end tabular-nums whitespace-nowrap' } satisfies Record<TableAlign, string>,
  },
  custom: {
    base: 'flex-row items-center',
    align: { start: 'justify-start', end: 'justify-end' } satisfies Record<TableAlign, string>,
  },
  // Stacked: one block a record. Its first line holds the leading field and, at the end edge, the
  // number, so numbers still line up down the table; the other fields wrap under it by whole pairs.
  block: {
    base: 'gap-1 px-4 self-stretch text-start',
    density: { comfortable: 'min-h-[44px] py-2.5 justify-center', compact: 'min-h-[36px] py-1.5 justify-center' } satisfies Record<TableDensity, string>,
  },
  lead: {
    // items-start, not items-baseline: View bug, a wrapped <text> in a baseline row is painted on one line.
    base: 'flex-row items-start gap-x-3 self-stretch',
  },
  leadCell: {
    // No basis-0: View bug, a zero-basis grow child keeps no width after its keyed row moves.
    base: 'grow shrink min-w-0 items-stretch',
  },
  figure: {
    base: 'shrink-0',
  },
  pairs: {
    base: 'flex-row flex-wrap items-center gap-x-4 gap-y-0.5 self-stretch',
  },
  pair: {
    base: 'flex-row items-center gap-x-1.5 max-w-full',
  },
  pairHeading: {
    base: 'shrink-0 text-footnote text-muted',
  },
  pairValue: {
    base: 'shrink text-footnote text-text',
  },
  empty: {
    base: 'px-4 py-6 text-body text-muted text-center',
  },
  /** A line of body type measured and never seen: how wide a character is at the reader's text size. */
  measure: {
    base: 'absolute size-0 overflow-hidden flex-row',
    sample: 'shrink-0 whitespace-nowrap text-body tabular-nums',
  },
} as const;
