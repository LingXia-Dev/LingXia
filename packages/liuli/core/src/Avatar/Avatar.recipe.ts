import type { AvatarGroupSurface, AvatarSize } from './Avatar';

export const avatarParts = {
  root: 'view',
  image: 'image',
  initials: 'text',
  group: 'view',
  overflow: 'text',
} as const;

/** A circle: the photo when it loads, else initials on the accent's soft step. */
export const avatarRecipe = {
  root: {
    base: 'relative shrink-0 items-center justify-center rounded-full bg-accent-soft',
    size: { xs: 'size-6', sm: 'size-8', md: 'size-10', lg: 'size-14' } satisfies Record<AvatarSize, string>,
    /** In a group, a 2 px edge in the colour of the surface behind cuts each one out of the next. */
    inGroup: {
      surface: 'border-2 border-surface',
      raised: 'border-2 border-surface-raised',
      sunken: 'border-2 border-surface-sunken',
    } satisfies Record<AvatarGroupSurface, string>,
  },
  image: {
    base: 'absolute inset-0 size-full rounded-full object-cover',
  },
  initials: {
    base: 'font-semibold text-on-accent-soft',
    size: { xs: 'text-caption', sm: 'text-caption', md: 'text-footnote', lg: 'text-callout' } satisfies Record<AvatarSize, string>,
  },
  group: {
    base: 'flex-row',
  },
  /** Every avatar after the first tucks under the edge of the one before. */
  overlap: {
    size: { xs: '-ms-1.5', sm: '-ms-2', md: '-ms-2.5', lg: '-ms-3.5' } satisfies Record<AvatarSize, string>,
  },
  /** Opaque, since the avatar before it shows through anything lighter. */
  overflow: {
    base: 'bg-line',
    text: 'text-text',
  },
} as const;
