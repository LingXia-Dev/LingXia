import type { Hue } from '../hue';
import type { BadgeSize, BadgeStatusTone, BadgeTone } from './Badge';

export const badgeParts = { root: 'view', icon: 'vector', dot: 'view', label: 'text' } as const;

/**
 * Badge classes. Soft status tints keep text in the text role; solid fills use their proven
 * `on-` role. Neutral has no solid form. A hue is its soft fill with a dot (or the icon) in the hue.
 */
export const badgeRecipe = {
  root: {
    base: 'flex-row items-center-safe self-start rounded-full',
    // A least height, so a label at large text makes the pill taller instead of spilling out.
    size: { sm: 'min-h-5 py-0.5 px-2 gap-1', md: 'min-h-6 py-0.5 px-2.5 gap-1.5' } satisfies Record<BadgeSize, string>,
    soft: {
      // A quiet tint of the text colour: it reads as a pill on any surface, where a sunken surface reads as a hole.
      neutral: 'bg-text/8 text-text',
      accent: 'bg-accent-soft text-on-accent-soft',
      info: 'bg-info-soft text-text',
      success: 'bg-success-soft text-text',
      warning: 'bg-warning-soft text-text',
      danger: 'bg-danger-soft text-text',
    } satisfies Record<BadgeTone, string>,
    solid: {
      accent: 'bg-accent text-on-accent',
      info: 'bg-info text-on-info',
      success: 'bg-success text-on-success',
      warning: 'bg-warning text-on-warning',
      danger: 'bg-danger text-on-danger',
    } satisfies Record<BadgeStatusTone, string>,
    hue: { 1: 'bg-hue-1-soft text-text', 2: 'bg-hue-2-soft text-text', 3: 'bg-hue-3-soft text-text', 4: 'bg-hue-4-soft text-text', 5: 'bg-hue-5-soft text-text', 6: 'bg-hue-6-soft text-text', 7: 'bg-hue-7-soft text-text', 8: 'bg-hue-8-soft text-text' } satisfies Record<Hue, string>,
  },
  icon: {
    base: 'shrink-0',
    size: { sm: 'size-3', md: 'size-3.5' } satisfies Record<BadgeSize, string>,
    hue: { 1: 'text-hue-1', 2: 'text-hue-2', 3: 'text-hue-3', 4: 'text-hue-4', 5: 'text-hue-5', 6: 'text-hue-6', 7: 'text-hue-7', 8: 'text-hue-8' } satisfies Record<Hue, string>,
  },
  /** The hue itself, as a mark: the fill alone is too pale to tell eight apart. */
  dot: {
    base: 'shrink-0 rounded-full',
    size: { sm: 'size-1.5', md: 'size-2' } satisfies Record<BadgeSize, string>,
    hue: { 1: 'bg-hue-1', 2: 'bg-hue-2', 3: 'bg-hue-3', 4: 'bg-hue-4', 5: 'bg-hue-5', 6: 'bg-hue-6', 7: 'bg-hue-7', 8: 'bg-hue-8' } satisfies Record<Hue, string>,
  },
  label: {
    base: 'font-medium',
    size: { sm: 'text-caption', md: 'text-footnote' } satisfies Record<BadgeSize, string>,
  },
} as const;
