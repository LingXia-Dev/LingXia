import type { Hue } from '../hue';
import type { TokenSize } from './Token';

export const tokenParts = {
  root: 'view',
  label: 'text',
  icon: 'vector',
  dot: 'view',
  open: 'button',
  remove: 'button',
  removeIcon: 'vector',
  list: 'view',
} as const;

/** A pill on a tint of the text colour: one chosen thing, with a small button that takes it away. */
export const tokenRecipe = {
  root: {
    base: 'flex-row items-center shrink-0 max-w-full rounded-full bg-text/8 text-text',
    size: { sm: 'h-6 ps-2.5 gap-1', md: 'h-8 ps-3 gap-1.5' } satisfies Record<TokenSize, string>,
    /** Without a remove button the end inset matches the start. */
    plain: { sm: 'pe-2.5', md: 'pe-3' } satisfies Record<TokenSize, string>,
    pressable: 'transition-colors duration-fast ease-standard hover:bg-text/12 active:bg-text/16 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    /** A category: the hue's soft fill; selected still wins. */
    hue: { 1: 'bg-hue-1-soft', 2: 'bg-hue-2-soft', 3: 'bg-hue-3-soft', 4: 'bg-hue-4-soft', 5: 'bg-hue-5-soft', 6: 'bg-hue-6-soft', 7: 'bg-hue-7-soft', 8: 'bg-hue-8-soft' } satisfies Record<Hue, string>,
    selected: 'bg-accent-soft text-on-accent-soft',
    disabled: 'opacity-40',
  },
  icon: {
    size: { sm: 'size-3.5 shrink-0', md: 'size-4 shrink-0' } satisfies Record<TokenSize, string>,
    hue: { 1: 'text-hue-1', 2: 'text-hue-2', 3: 'text-hue-3', 4: 'text-hue-4', 5: 'text-hue-5', 6: 'text-hue-6', 7: 'text-hue-7', 8: 'text-hue-8' } satisfies Record<Hue, string>,
  },
  dot: {
    base: 'size-2 shrink-0 rounded-full',
    hue: { 1: 'bg-hue-1', 2: 'bg-hue-2', 3: 'bg-hue-3', 4: 'bg-hue-4', 5: 'bg-hue-5', 6: 'bg-hue-6', 7: 'bg-hue-7', 8: 'bg-hue-8' } satisfies Record<Hue, string>,
  },
  label: {
    base: 'shrink min-w-0 truncate',
    size: { sm: 'text-caption font-medium', md: 'text-footnote font-medium' } satisfies Record<TokenSize, string>,
  },
  /** The label as its own button, beside the remove button. */
  open: {
    base: 'flex-row items-center shrink min-w-0 rounded-full focus-visible:ring-2 focus-visible:ring-focus',
    size: { sm: 'h-6 gap-1', md: 'h-8 gap-1.5' } satisfies Record<TokenSize, string>,
  },
  remove: {
    base: 'items-center justify-center rounded-full text-muted hover:bg-text/10 hover:text-text active:bg-text/16 focus-visible:ring-2 focus-visible:ring-focus',
    size: { sm: 'size-6', md: 'size-8' } satisfies Record<TokenSize, string>,
  },
  removeIcon: {
    size: { sm: 'size-3', md: 'size-3.5' } satisfies Record<TokenSize, string>,
  },
  /** TokenList: tokens in a row that wraps. */
  list: {
    base: 'flex-row flex-wrap items-center gap-1.5',
  },
} as const;
