import type { QRCodeSize } from './QRCode';

export const qrCodeParts = { root: 'view', code: 'vector' } as const;

/** A square tile, dark modules on light with the quiet zone inside it, in both themes. */
export const qrCodeRecipe = {
  root: {
    base: 'shrink-0 overflow-hidden rounded-control',
    size: { sm: 'size-24', md: 'size-40', lg: 'size-60' } satisfies Record<QRCodeSize, string>,
    /** No code: the value is too long for it. */
    empty: 'bg-text/6',
  },
  code: {
    base: 'size-full',
  },
} as const;
