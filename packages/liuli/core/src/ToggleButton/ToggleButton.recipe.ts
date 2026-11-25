import type { ToggleButtonSize } from './ToggleButton';

export const toggleButtonParts = { root: 'button', glyph: 'vector', label: 'text' } as const;

/**
 * A secondary button that stays down while on: the soft accent fill and an accent hairline, so the
 * state is in the fill and the edge, not the label's colour alone. Disabled keeps that fill when on,
 * so a locked option still shows its state.
 */
export const toggleButtonRecipe = {
  root: {
    base: 'flex-row items-center justify-center-safe gap-1.5 rounded-control border transition-colors duration-fast ease-standard focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    off: 'bg-surface text-text border-line shadow-control hover:bg-text/5 active:bg-text/8 disabled:bg-text/6 disabled:text-muted disabled:border-transparent disabled:shadow-none',
    /** Off inside a dialog, sheet or popover: the overlay's surface. */
    overlay: 'bg-surface-overlay',
    on: 'bg-accent-soft text-text border-accent hover:bg-accent/16 active:bg-accent/20 disabled:bg-accent-soft/50 disabled:text-faint disabled:border-transparent',
    size: {
      sm: 'min-h-control-sm px-3',
      md: 'min-h-control-md px-4',
      lg: 'min-h-control-lg px-5',
    } satisfies Record<ToggleButtonSize, string>,
    /** Icon-only: a square the height of its size, grown with the glyph. */
    square: {
      sm: 'size-control-sm px-0',
      md: 'size-control-md px-0',
      lg: 'size-control-lg px-0',
    } satisfies Record<ToggleButtonSize, string>,
  },
  glyph: {
    size: { sm: 'size-4 shrink-0', md: 'size-[1.125rem] shrink-0', lg: 'size-5 shrink-0' } satisfies Record<ToggleButtonSize, string>,
  },
  label: {
    base: 'font-medium text-start',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<ToggleButtonSize, string>,
  },
} as const;
