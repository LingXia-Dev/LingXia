import type { DialogSize } from './Dialog';

export const dialogParts = {
  panel: 'dialog',
  header: 'view',
  title: 'text',
  close: 'button',
  closeIcon: 'vector',
  description: 'text',
  body: 'scroll',
  footer: 'view',
} as const;

/**
 * A centred modal panel. It enters and leaves by fading and scaling from 95%; under reduced motion
 * it only fades. The backdrop is the primitive's `bg-scrim`.
 */
export const dialogRecipe = {
  panel: {
    base: 'w-[90%] max-h-[85vh] p-5 gap-4 rounded-dialog border border-text/12 bg-surface-overlay shadow-modal transition duration-200 ease-out starting:opacity-0 exit:opacity-0 motion-safe:starting:scale-96 motion-safe:exit:scale-96',
    size: { sm: 'max-w-[360px]', md: 'max-w-[480px]', lg: 'max-w-[640px]' } satisfies Record<DialogSize, string>,
  },
  header: {
    base: 'flex-row items-start gap-3',
  },
  title: {
    base: 'grow basis-0 text-title font-semibold tracking-tight text-text',
  },
  close: {
    base: 'size-8 -me-1.5 -mt-1 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  closeIcon: {
    base: 'size-5',
  },
  description: {
    base: 'text-body text-muted',
  },
  body: {
    base: 'shrink min-h-0 gap-3',
  },
  footer: {
    base: 'flex-row flex-wrap justify-end gap-2',
  },
} as const;
