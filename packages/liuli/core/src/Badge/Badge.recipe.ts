import type { BadgeSize, BadgeStatusTone, BadgeTone } from './Badge';

export const badgeParts = { root: 'view', icon: 'view', label: 'text' } as const;

/**
 * Badge classes. Soft status tints keep text in the text role; solid fills use their proven
 * `on-` role. Neutral has no solid form.
 */
export const badgeRecipe = {
  root: {
    base: 'flex-row items-center-safe self-start rounded-full',
    size: { sm: 'h-5 px-2 gap-1', md: 'h-6 px-2.5 gap-1.5' } satisfies Record<BadgeSize, string>,
    soft: {
      neutral: 'bg-surface-sunken text-text',
      accent: 'bg-accent-soft text-on-accent-soft',
      success: 'bg-success-soft text-text',
      warning: 'bg-warning-soft text-text',
      danger: 'bg-danger-soft text-text',
    } satisfies Record<BadgeTone, string>,
    solid: {
      accent: 'bg-accent text-on-accent',
      success: 'bg-success text-on-success',
      warning: 'bg-warning text-on-warning',
      danger: 'bg-danger text-on-danger',
    } satisfies Record<BadgeStatusTone, string>,
  },
  icon: {
    base: 'items-center justify-center',
    size: { sm: 'size-3', md: 'size-3.5' } satisfies Record<BadgeSize, string>,
  },
  label: {
    base: 'font-medium',
    size: { sm: 'text-caption', md: 'text-footnote' } satisfies Record<BadgeSize, string>,
  },
} as const;
