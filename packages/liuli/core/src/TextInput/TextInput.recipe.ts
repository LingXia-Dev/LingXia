import type { FieldSize, FieldStatusType } from '../Field/Field';

export const textInputParts = { box: 'view', icon: 'view', searchIcon: 'vector', input: 'input', clear: 'button', clearIcon: 'vector' } as const;

/**
 * An editor keeps a 3:1 boundary (`line-strong`) so it never reads as a button. The box draws the border, so a leading icon and the clear button sit inside it, and takes the
 * focus colour through `focus-within:`. The editor is borderless and fills the box height, so a tap
 * anywhere but the icon lands in it.
 */
export const textInputRecipe = {
  box: {
    base: 'flex-row items-center gap-2 px-3 rounded-control border border-line-strong bg-surface transition duration-150 ease-out focus-within:border-accent focus-within:ring-3 focus-within:ring-accent/20',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-line',
  },
  icon: {
    base: 'size-4 items-center justify-center text-muted',
  },
  searchIcon: {
    base: 'size-4',
  },
  input: {
    /** The box owns the inset, so the editor's own padding is zeroed. */
    base: 'grow basis-0 min-w-0 self-stretch px-0 bg-transparent text-text placeholder:text-muted disabled:text-faint',
    /** Matches the Button label at each size. */
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<FieldSize, string>,
  },
  clear: {
    base: 'size-8 -me-1.5 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus',
  },
  clearIcon: {
    base: 'size-4',
  },
} as const;
