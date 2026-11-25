import type { KbdSize } from './Kbd';

export const kbdParts = {
  root: 'view',
  key: 'view',
  label: 'text',
} as const;

/** Key caps: a quiet outlined box per key, in the text face so symbols and letters sit together. */
export const kbdRecipe = {
  root: {
    base: 'flex-row items-center gap-1 shrink-0',
  },
  key: {
    base: 'items-center justify-center rounded-segment border border-line bg-surface',
    size: { sm: 'h-5 min-w-5 px-1', md: 'h-6 min-w-6 px-1.5' } satisfies Record<KbdSize, string>,
  },
  label: {
    base: 'font-medium text-muted',
    size: { sm: 'text-caption', md: 'text-footnote' } satisfies Record<KbdSize, string>,
  },
} as const;
