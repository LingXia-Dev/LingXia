import type { IconSize } from './Icon';

export const iconParts = {
  box: 'view',
  glyph: 'vector',
} as const;

/**
 * An icon is a square that never shrinks and draws in the text colour around it. A labelled icon
 * sits in a box that carries the label; an unlabelled one is the drawing alone.
 */
export const iconRecipe = {
  box: {
    base: 'shrink-0',
    size: { sm: 'size-4', md: 'size-5', lg: 'size-6', inline: 'size-[1.33em]' } satisfies Record<IconSize, string>,
  },
  glyph: {
    base: 'shrink-0',
    size: { sm: 'size-4', md: 'size-5', lg: 'size-6', inline: 'size-[1.33em]' } satisfies Record<IconSize, string>,
    mirrored: 'rtl:-scale-x-100',
  },
} as const;
