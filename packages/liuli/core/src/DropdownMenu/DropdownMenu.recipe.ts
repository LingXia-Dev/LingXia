import type { ButtonSize } from '../Button/Button';
import type { DropdownMenuItemVariant, DropdownMenuPlacement } from './DropdownMenu';

export const dropdownMenuParts = {
  chevron: 'vector',
  menu: 'view',
  items: 'view',
  item: 'button',
  itemIcon: 'view',
  itemText: 'view',
  itemLabel: 'text',
  itemDescription: 'text',
  itemCheck: 'vector',
  submenuChevron: 'vector',
  back: 'button',
  radioGroup: 'view',
  group: 'view',
  groupLabel: 'text',
  divider: 'view',
} as const;

/** The trigger reuses `buttonRecipe`; these are its chevron and the menu itself. */
export const dropdownMenuRecipe = {
  chevron: {
    base: 'size-4 -me-1',
  },
  menu: {
    base: 'min-w-48 p-1 gap-0.5 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0 motion-safe:starting:scale-95',
    /** The gap between the trigger and the menu, on the side the menu opens to. */
    offset: { bottom: 'mt-1 origin-top', top: 'mb-1 origin-bottom' } satisfies Record<DropdownMenuPlacement, string>,
    /** A submenu overlaps its parent by the parent's padding, so the two read as one piece. */
    side: '-ms-1',
    /** Where a submenu takes the panel, the panel changes height around it. */
    drill: 'overflow-hidden transition-[height]',
  },
  /** The items, at the menu's own rhythm. */
  items: {
    base: 'gap-0.5',
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
  /** The check at the end of a checked checkbox or radio item. */
  itemCheck: {
    base: 'size-4 shrink-0 text-accent',
  },
  /** Spaced as the menu spaces its items. */
  radioGroup: {
    base: 'gap-0.5',
  },
  group: {
    base: 'gap-0.5',
  },
  /** A quiet heading at the items' inset. */
  groupLabel: {
    base: 'px-3 pt-1.5 pb-1 text-caption font-medium text-muted',
  },
  /** Points to where the submenu opens; mirrored for right-to-left text by the icon itself. */
  submenuChevron: {
    base: 'size-4 shrink-0 text-muted',
  },
  /** The first row of a submenu that has taken the panel: it names the submenu and goes back. */
  back: {
    base: 'gap-2',
    label: 'grow basis-0 min-w-0 truncate text-body font-medium',
  },
  divider: {
    base: 'h-px -mx-1 my-1 bg-text/8',
  },
} as const;
