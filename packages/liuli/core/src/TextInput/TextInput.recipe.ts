export const textInputParts = { box: 'view', icon: 'view', input: 'input', clear: 'button', clearIcon: 'vector' } as const;

/**
 * The box draws the border, so a leading icon and the clear button sit inside it, and takes the
 * focus colour through `focus-within:`. The editor is borderless and fills the box height, so a tap
 * anywhere but the icon lands in it.
 */
export const textInputRecipe = {
  box: {
    base: 'flex-row items-center gap-2 px-3 rounded-control border border-line-strong bg-surface transition-colors duration-150 focus-within:border-focus',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' },
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' },
    disabled: 'bg-surface-sunken border-line',
  },
  icon: {
    base: 'items-center justify-center text-muted',
  },
  input: {
    base: 'grow basis-0 min-w-0 self-stretch bg-transparent text-text placeholder:text-muted disabled:text-faint',
    size: { sm: 'text-sm', md: 'text-base', lg: 'text-base' },
  },
  clear: {
    base: 'size-8 -me-1.5 items-center justify-center rounded-full text-muted hover:bg-surface-sunken active:bg-surface-sunken',
  },
  clearIcon: {
    base: 'size-4',
  },
} as const;
