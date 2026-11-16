import type { PageControlTone } from './PageControl';

export const pageControlParts = {
  root: 'view',
  dot: 'button',
  mark: 'view',
} as const;

/** Small marks in finger-sized buttons: the current one stretches into a pill and takes the tone. */
export const pageControlRecipe = {
  root: {
    base: 'flex-row items-center justify-center self-center',
  },
  dot: {
    base: 'px-1 py-2.5 items-center justify-center rounded-full focus-visible:ring-2 focus-visible:ring-focus',
  },
  mark: {
    base: 'h-1.5 rounded-full transition-layout duration-base ease-standard',
    state: { on: 'w-4.5', off: 'w-1.5' },
    tone: {
      accent: { on: 'bg-accent', off: 'bg-text/20' },
      'on-media': { on: 'bg-on-media', off: 'bg-on-media/40' },
    } satisfies Record<PageControlTone, { on: string; off: string }>,
  },
} as const;
