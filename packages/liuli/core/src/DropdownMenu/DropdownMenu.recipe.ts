import type { ButtonSize } from '../Button/Button';
import type { DropdownMenuItemVariant, DropdownMenuPlacement } from './DropdownMenu';

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
    base: 'min-w-48 p-1 gap-0.5 rounded-container border border-text/12 bg-surface-raised shadow-raised transition duration-150 ease-out starting:opacity-0 exit:opacity-0 motion-safe:starting:scale-95',
    /** The gap between the trigger and the menu, on the side the menu opens to. */
    offset: { bottom: 'mt-1 origin-top', top: 'mb-1 origin-bottom' } satisfies Record<DropdownMenuPlacement, string>,
  },
  item: {
    /** Concentric with the menu: its radius less the menu's padding. */
    base: 'flex-row items-center gap-3 px-3 py-2 rounded-control text-start text-text hover:bg-text/5 active:bg-text/8 focus-visible:bg-text/8 disabled:opacity-40',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<ButtonSize, string>,
  },
  itemIcon: {
    base: 'items-center justify-center',
    variant: { default: 'text-muted', destructive: 'text-danger' } satisfies Record<DropdownMenuItemVariant, string>,
  },
  itemText: {
    base: 'grow basis-0 gap-0.5',
  },
  itemLabel: {
    base: 'text-body',
  },
  itemDescription: {
    base: 'text-footnote text-muted',
  },
  divider: {
    base: 'h-px -mx-1 my-1 bg-line',
  },
} as const;
