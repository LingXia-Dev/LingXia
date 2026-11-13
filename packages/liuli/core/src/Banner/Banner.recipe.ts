import type { BannerTone } from './Banner';

export const bannerParts = {
  root: 'view',
  icon: 'vector',
  text: 'view',
  title: 'text',
  message: 'text',
  action: 'Button',
  dismiss: 'button',
} as const;

/**
 * A soft tint of the tone with a hairline of it; the text stays in the text role, as soft status
 * tints are not proven for small coloured text. The icon carries the tone.
 */
export const bannerRecipe = {
  root: {
    base: 'flex-row items-start gap-3 rounded-container border ps-4 pe-2 py-3',
    tone: {
      neutral: 'bg-surface-sunken border-text/10',
      info: 'bg-accent-soft border-accent/20',
      success: 'bg-success-soft border-success/25',
      warning: 'bg-warning-soft border-warning/30',
      danger: 'bg-danger-soft border-danger/25',
    } satisfies Record<BannerTone, string>,
    /** Without a dismiss button the end padding matches the start. */
    plain: 'pe-4',
  },
  icon: {
    base: 'size-5 shrink-0 mt-px',
    tone: { neutral: 'text-muted', info: 'text-accent', success: 'text-success', warning: 'text-warning', danger: 'text-danger' } satisfies Record<BannerTone, string>,
  },
  text: {
    base: 'grow shrink basis-0 min-w-0 gap-0.5 items-start',
  },
  title: {
    base: 'text-body font-semibold text-text',
  },
  message: {
    base: 'text-footnote text-text',
  },
  action: {
    /** The link's own padding moves out, so its text lines up with the message and the bottom
     * inset matches the top. */
    base: 'mt-1 -ms-3 -mb-1.5',
  },
  dismiss: {
    base: 'size-8 -mt-1 shrink-0 items-center justify-center rounded-full text-muted hover:bg-text/6 active:bg-text/10 focus-visible:ring-2 focus-visible:ring-focus',
  },
  dismissIcon: {
    base: 'size-4',
  },
} as const;
