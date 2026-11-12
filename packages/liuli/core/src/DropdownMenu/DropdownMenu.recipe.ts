export const dropdownMenuParts = {
  chevron: 'vector',
  menu: 'view',
  item: 'button',
  itemIcon: 'view',
  itemText: 'view',
  itemLabel: 'text',
  itemDescription: 'text',
  divider: 'view',
} as const;

/** The trigger reuses `buttonRecipe`; these are its chevron and the menu itself. */
export const dropdownMenuRecipe = {
  chevron: {
    base: 'size-4 -me-1',
  },
  menu: {
    base: 'min-w-48 p-1 gap-0.5 rounded-container border border-line bg-surface-raised shadow-lg transition duration-150 ease-out starting:opacity-0 exit:opacity-0 motion-safe:starting:scale-95',
    /** The gap between the trigger and the menu, on the side the menu opens to. */
    offset: { bottom: 'mt-1', top: 'mb-1' },
  },
  item: {
    base: 'flex-row items-center gap-3 px-3 py-2 rounded-lg text-start text-text hover:bg-surface-sunken active:bg-surface-sunken disabled:opacity-40',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' },
  },
  itemIcon: {
    base: 'items-center justify-center',
    variant: { default: 'text-muted', destructive: 'text-danger' },
  },
  itemText: {
    base: 'grow basis-0 gap-0.5',
  },
  itemLabel: {
    base: 'text-sm',
  },
  itemDescription: {
    base: 'text-xs text-muted',
  },
  divider: {
    base: 'h-px -mx-1 my-1 bg-line',
  },
} as const;
