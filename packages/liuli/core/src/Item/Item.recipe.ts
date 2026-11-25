import type { Layer } from '../internal/layer';
import type { ItemDensity, ItemSwipeVariant, ItemVariant } from './Item';

export const itemParts = {
  group: 'view',
  title: 'text',
  note: 'text',
  list: 'view',
  listitem: 'view',
  root: 'view',
  iconTile: 'view',
  start: 'view',
  text: 'view',
  labelLine: 'view',
  label: 'text',
  labelEnd: 'view',
  description: 'text',
  status: 'view',
  end: 'view',
  endAction: 'button',
  chevron: 'vector',
  swipe: 'view',
  slide: 'view',
  actions: 'view',
  action: 'button',
  actionIcon: 'vector',
  actionLabel: 'text',
} as const;

/** List row classes by part. A pressable row lands on a `button`, a plain one on a `view`. */
export const itemRecipe = {
  group: {
    base: 'self-stretch gap-1.5',
  },
  title: {
    base: 'text-footnote font-semibold text-muted',
  },
  note: {
    base: 'text-footnote text-muted',
  },
  list: {
    base: 'divide-y divide-line',
    /** Its own border and corners, unless it fills a pane that has them. */
    framed: 'overflow-hidden rounded-container border border-line',
    /** One rung up from what it sits on; inside an overlay it takes the overlay's surface. */
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
  },
  listitem: {
    base: 'self-stretch',
  },
  root: {
    base: 'flex-row items-center gap-3 px-4 self-stretch text-start',
    density: { comfortable: 'py-2.5 min-h-control-lg', compact: 'py-1.5 min-h-control-md' } satisfies Record<ItemDensity, string>,
    pressable: 'transition-colors duration-fast ease-standard disabled:opacity-40',
    idle: 'hover:bg-text/4 active:bg-text/7 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
    /** The current choice: ToggleButton's on wash, deepening on hover, press and focus. */
    selected: 'bg-accent-soft hover:bg-accent/16 active:bg-accent/20 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
  },
  /** One tint for every row, so a list of them reads as a set and the icons carry the difference. */
  iconTile: {
    base: 'size-8 shrink-0 items-center justify-center rounded-control bg-accent-soft text-on-accent-soft',
    icon: 'size-[18px]',
  },
  start: {
    base: 'shrink-0 items-center justify-center text-muted',
  },
  /** The text gives way to the end content: an unbroken word such as an address breaks rather than push it out. */
  text: {
    base: 'grow shrink basis-0 min-w-0 gap-0.5',
  },
  label: {
    base: 'text-body break-words',
    variant: {
      default: 'text-text',
      destructive: 'text-danger',
    } satisfies Record<ItemVariant, string>,
    selected: 'font-medium',
    /** Beside `labelEnd` the name keeps one line and truncates; the content after it stays whole. */
    beside: 'shrink min-w-0 truncate break-normal',
    /** On its own line the name takes the line's width, so it still truncates there. */
    stacked: 'self-stretch',
  },
  labelLine: {
    base: 'flex-row items-center gap-2 min-w-0',
    /** The name kept whole enough to read: labelEnd on its own line under it. */
    stacked: 'flex-col items-start gap-0.5',
  },
  labelEnd: {
    base: 'shrink-0 flex-row items-center gap-1',
  },
  /** A label-hidden control's failure, under the row's words, until its next change. */
  status: {
    base: 'mt-0.5',
  },
  description: {
    base: 'text-footnote text-muted break-words whitespace-pre-line',
    node: 'gap-1 text-footnote text-muted',
  },
  end: {
    base: 'shrink-0 flex-row items-center gap-2 text-muted',
    under: 'flex-row flex-wrap items-center gap-2 text-muted',
  },
  endAction: {
    base: 'shrink-0',
    variant: { default: 'text-muted', destructive: 'text-danger' } satisfies Record<ItemVariant, string>,
  },
  swipe: {
    base: 'relative self-stretch overflow-hidden',
  },
  slide: {
    base: 'self-stretch transition-transform duration-slow ease-standard',
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
  },
  actions: {
    base: 'absolute inset-y-0 end-0 flex-row items-stretch',
  },
  action: {
    base: 'w-[72px] items-center justify-center gap-0.5 px-1 transition-colors duration-fast ease-standard focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
    variant: {
      default: 'bg-text/8 text-text hover:bg-text/12 active:bg-text/16',
      accent: 'bg-accent-solid text-on-accent-solid hover:bg-accent-solid-hover active:bg-accent-solid-pressed',
      destructive: 'bg-danger-solid text-on-danger-solid hover:bg-danger-solid-hover active:bg-danger-solid-pressed',
    } satisfies Record<ItemSwipeVariant, string>,
  },
  actionIcon: {
    base: 'size-5 shrink-0',
  },
  actionLabel: {
    base: 'text-caption font-medium text-center',
  },
  chevron: {
    base: 'size-5 shrink-0 text-faint',
    /** Faint is below 3:1 on the selected wash. */
    selected: 'text-muted',
  },
} as const;
