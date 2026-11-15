import type { Layer } from '../internal/layer';
import type { ItemDensity, ItemSwipeTone, ItemTone } from './Item';

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
  label: 'text',
  description: 'text',
  end: 'view',
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
    base: 'px-4 text-footnote font-medium text-muted',
  },
  note: {
    base: 'px-4 text-footnote text-muted',
  },
  list: {
    base: 'overflow-hidden rounded-container border border-text/12 divide-y divide-text/8',
    /** One rung up from what it sits on; inside an overlay it takes the overlay's surface. */
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
  },
  listitem: {
    base: 'self-stretch',
  },
  root: {
    base: 'flex-row items-center gap-3 px-4 self-stretch text-start',
    density: { comfortable: 'py-2.5 min-h-control-lg', compact: 'py-1.5 min-h-control-md' } satisfies Record<ItemDensity, string>,
    pressable: 'transition-colors duration-150 ease-out disabled:opacity-40',
    idle: 'hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7',
    /** The current choice: ToggleButton's on wash, deepening on hover, press and focus. */
    selected: 'bg-accent-soft hover:bg-accent/16 active:bg-accent/20 focus-visible:bg-accent/20',
  },
  /** One tint for every row, so a list of them reads as a set and the icons carry the difference. */
  iconTile: {
    base: 'size-8 shrink-0 items-center justify-center rounded-control bg-accent-soft text-on-accent-soft',
    icon: 'size-[18px]',
  },
  start: {
    base: 'shrink-0 items-center justify-center text-muted',
  },
  text: {
    base: 'grow shrink basis-0 gap-0.5',
  },
  label: {
    base: 'text-body',
    tone: {
      default: 'text-text',
      danger: 'text-danger',
    } satisfies Record<ItemTone, string>,
    selected: 'font-medium',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  end: {
    base: 'shrink-0 flex-row items-center gap-2 text-muted',
  },
  swipe: {
    base: 'relative self-stretch overflow-hidden',
  },
  slide: {
    base: 'self-stretch transition-transform duration-300 ease-out',
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
  },
  actions: {
    base: 'absolute inset-y-0 end-0 flex-row items-stretch',
  },
  action: {
    base: 'w-[72px] items-center justify-center gap-0.5 px-1 transition-colors duration-150 ease-out focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
    tone: {
      neutral: 'bg-text/8 text-text hover:bg-text/12 active:bg-text/16',
      accent: 'bg-accent text-on-accent hover:bg-accent/92 active:bg-accent/88',
      danger: 'bg-danger text-on-danger hover:bg-danger/92 active:bg-danger/88',
    } satisfies Record<ItemSwipeTone, string>,
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
