import type { PopoverPlacement } from './Popover';

export const popoverParts = {
  panel: 'view',
  title: 'text',
} as const;

/**
 * The trigger reuses `buttonRecipe`. The panel is a raised card beside the trigger, as the
 * DropdownMenu's menu is drawn, roomier for content; it fades and grows from the trigger's side.
 */
export const popoverRecipe = {
  panel: {
    base: 'w-72 p-4 gap-3 rounded-container border border-text/12 bg-surface-raised shadow-raised transition duration-150 ease-out starting:opacity-0 exit:opacity-0 motion-safe:starting:scale-95',
    /** The gap between the trigger and the panel, on the side it opens to. */
    offset: { bottom: 'mt-1.5 origin-top', top: 'mb-1.5 origin-bottom' } satisfies Record<PopoverPlacement, string>,
  },
  title: {
    base: 'text-body font-semibold text-text',
  },
} as const;
