import type { PopoverPlacement } from './Popover';

export const popoverParts = {
  panel: 'view',
  title: 'text',
} as const;

/**
 * The trigger is the author's own Button. The panel is a raised card beside the trigger, as the
 * DropdownMenu's menu is drawn, roomier for content; it fades and grows from the trigger's side.
 */
export const popoverRecipe = {
  panel: {
    base: 'w-72 p-4 gap-3 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    /** The gap between the trigger and the panel, on the side it opens to. */
    offset: { bottom: 'mt-1.5 motion-safe:starting:-translate-y-1', top: 'mb-1.5 motion-safe:starting:translate-y-1', end: 'ms-1.5 motion-safe:starting:-translate-x-1 rtl:motion-safe:starting:translate-x-1', start: 'me-1.5 motion-safe:starting:translate-x-1 rtl:motion-safe:starting:-translate-x-1' } satisfies Record<PopoverPlacement, string>,
  },
  title: {
    base: 'text-body font-semibold text-text',
  },
} as const;
