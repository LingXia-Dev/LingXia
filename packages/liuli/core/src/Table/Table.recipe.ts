import type { Layer } from '../internal/layer';
import type { TableAlign, TableDensity } from './Table';

export const tableParts = {
  root: 'view',
  header: 'view',
  heading: 'text',
  sortHeading: 'button',
  sortMark: 'view',
  sortBar: 'view',
  sortGroup: 'view',
  select: 'view',
  selectable: 'view',
  inset: 'view',
  loading: 'view',
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
    base: 'grid items-center gap-x-3',
    /** The hairline under the headings, on the headings or on the line that holds them and the checkbox for all. */
    line: 'border-b border-line self-stretch',
    grow: 'grow shrink min-w-0',
    select: 'flex-row items-stretch',
    density: { comfortable: 'min-h-control-md', compact: 'min-h-control-sm' } satisfies Record<TableDensity, string>,
    // The rows pass under a heading that stays, so it carries the table's surface.
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
    sticky: 'sticky top-0 z-10',
  },
  /** The row's sides: its own inset, or, after a row's checkbox, the gap a column keeps from the next. */
  inset: {
    full: 'px-4',
    afterSelect: 'ps-3 pe-4',
    /** The sort bar after the checkbox for all, whose cell brings its own start inset. */
    select: 'pe-4',
  },
  /** A row's checkbox, in its own cell before the row's grid, centred on the row or the whole record. */
  select: {
    base: 'shrink-0 justify-center ps-4',
  },
  /** A selectable row: the checkbox's cell, then the row; the checked tint covers both. */
  selectable: {
    base: 'flex-row items-stretch self-stretch',
    checked: 'bg-accent-soft',
    rest: 'grow shrink min-w-0',
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
    base: 'flex-row items-center gap-3 py-1 border-b border-line self-stretch',
  },
  sortGroup: {
    base: 'flex-row items-center gap-1',
  },
  body: {
    base: 'self-stretch divide-y divide-line',
  },
  rowgroup: {
    base: 'self-stretch',
  },
  row: {
    base: 'grid items-center gap-x-3 self-stretch text-start',
    density: { comfortable: 'min-h-11 py-2', compact: 'min-h-9 py-1.5' } satisfies Record<TableDensity, string>,
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
    figureUnder: 'text-start tabular-nums',
    align: { start: 'text-start', end: 'text-end tabular-nums whitespace-nowrap' } satisfies Record<TableAlign, string>,
  },
  custom: {
    base: 'flex-row items-center',
    align: { start: 'justify-start', end: 'justify-end' } satisfies Record<TableAlign, string>,
  },
  // Stacked: one block a record. Its first line holds the leading field and, at the end edge, the
  // number, so numbers still line up down the table; the other fields wrap under it by whole pairs.
  block: {
    base: 'gap-1 self-stretch text-start',
    density: { comfortable: 'min-h-11 py-2.5 justify-center', compact: 'min-h-9 py-1.5 justify-center' } satisfies Record<TableDensity, string>,
  },
  lead: {
    base: 'flex-row items-baseline gap-x-3 self-stretch',
    /** At accessibility text sizes: the name, then the figure under it. */
    large: 'gap-1 self-stretch',
  },
  leadCell: {
    base: 'grow shrink basis-0 min-w-0 items-stretch',
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
  /** Placeholder bars while the rows load: text bars of varied widths, a number's short and at the end. */
  loading: {
    width: { long: 'w-3/4', half: 'w-1/2', most: 'w-2/3', short: 'w-2/5' },
    end: 'self-end w-14',
    /** The rows divide themselves, as the loaded ones do, without the placeholder group's own gap. */
    group: 'gap-0',
    figure: 'shrink-0 w-16',
    /** At accessibility text sizes, under the name bar, as the loaded figure goes. */
    figureUnder: 'self-start w-16',
    /** The bone's shape; its size is the small checkbox's own box, so the two match at every text size. */
    box: 'rounded-checkbox',
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
