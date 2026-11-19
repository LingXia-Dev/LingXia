import type { ButtonSize } from '../Button/Button';
import type { DropdownMenuItemVariant, DropdownMenuPlacement } from './DropdownMenu';

export const dropdownMenuParts = {
  menu: 'view',
  items: 'view',
  item: 'button',
  itemIcon: 'vector',
  itemText: 'view',
  itemLabel: 'text',
  itemDescription: 'text',
  itemCheck: 'vector',
  itemDash: 'view',
  submenuChevron: 'vector',
  back: 'button',
  radioGroup: 'view',
  group: 'view',
  groupLabel: 'text',
  divider: 'view',
} as const;

/** The menu and its items; the trigger is the author's own Button. */
export const dropdownMenuRecipe = {
  menu: {
    base: 'min-w-48 p-1 gap-0.5 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    /** The gap between the trigger and the menu, on the side the menu opens to. */
    offset: { bottom: 'mt-1 motion-safe:starting:-translate-y-1', top: 'mb-1 motion-safe:starting:translate-y-1' } satisfies Record<DropdownMenuPlacement, string>,
    /** A submenu overlaps its parent by the parent's padding, so the two read as one piece. */
    side: '-ms-1 motion-safe:starting:-translate-x-1 rtl:motion-safe:starting:translate-x-1',
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
    base: 'size-4 shrink-0',
    variant: { default: 'text-muted', destructive: 'text-danger' } satisfies Record<DropdownMenuItemVariant, string>,
  },
  itemText: {
    base: 'grow basis-0 gap-0.5',
  },
  itemLabel: {
    base: 'text-body',
    /** A destructive item is red in word and icon, as menus are on every platform. */
    variant: { default: '', destructive: 'text-danger' } satisfies Record<DropdownMenuItemVariant, string>,
  },
  itemDescription: {
    base: 'text-footnote text-muted',
  },
  /** The check at the end of a checked checkbox or radio item. */
  itemCheck: {
    base: 'size-4 shrink-0 text-accent',
  },
  /** Partly on: a dash where the check goes, in the check's colour and width. */
  itemDash: {
    base: 'w-2.5 h-0.5 mx-[3px] shrink-0 rounded-full bg-accent',
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
