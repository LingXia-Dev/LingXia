/** The element each part lands on. */
export const buttonParts = { root: 'button', icon: 'view', spinner: 'vector', label: 'text' } as const;

/**
 * Button classes by part, then by prop value. Exported so an app can style a raw `<button>` (a
 * `popoverTarget` invoker, say) to match.
 */
export const buttonRecipe = {
  root: {
    base: 'flex-row items-center justify-center rounded-control transition-colors duration-150 disabled:opacity-40',
    variant: {
      primary: 'bg-accent text-on-accent hover:opacity-90 active:opacity-80',
      secondary: 'bg-surface text-text border border-line-strong hover:bg-surface-sunken active:bg-surface-sunken',
      ghost: 'bg-transparent text-text hover:bg-surface-sunken active:bg-surface-sunken',
      destructive: 'bg-danger text-on-danger hover:opacity-90 active:opacity-80',
    },
    size: {
      sm: 'min-h-control-sm px-3 gap-1.5',
      md: 'min-h-control-md px-4 gap-2',
      lg: 'min-h-control-lg px-5 gap-2',
    },
    /** Square, for a trigger that shows only its icon. */
    square: {
      sm: 'size-control-sm px-0',
      md: 'size-control-md px-0',
      lg: 'size-control-lg px-0',
    },
    width: {
      content: 'self-start',
      fill: 'self-stretch',
    },
  },
  icon: {
    base: 'items-center justify-center',
  },
  spinner: {
    base: 'motion-safe:animate-spin',
    size: { sm: 'size-4', md: 'size-4', lg: 'size-5' },
  },
  label: {
    base: 'font-medium text-center',
    size: { sm: 'text-sm', md: 'text-sm', lg: 'text-base' },
  },
} as const;
