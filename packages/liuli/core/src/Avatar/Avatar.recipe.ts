import type { Hue } from '../hue';
import type { AvatarShape, AvatarGroupSurface, AvatarSize, AvatarStatus } from './Avatar';

export const avatarParts = {
  root: 'view',
  image: 'image',
  initials: 'text',
  group: 'view',
  overflow: 'text',
  status: 'view',
  badge: 'view',
} as const;

/** A circle: the photo when it loads, else initials on a translucent neutral fill. */
export const avatarRecipe = {
  root: {
    /** Translucent, so the fill is a step off whatever surface it sits on, with no border. */
    base: 'relative shrink-0 items-center-safe justify-center-safe bg-text/14',
    /** A rounded square's corner grows with its size, a quarter of it: the four smallest radius steps, by the names Liuli gives them. */
    shape: {
      circle: { xs: 'rounded-full', sm: 'rounded-full', md: 'rounded-full', lg: 'rounded-full' },
      rounded: { xs: 'rounded-checkbox', sm: 'rounded-segment', md: 'rounded-control', lg: 'rounded-container' },
    } satisfies Record<AvatarShape, Record<AvatarSize, string>>,
    /** A square at every text size; at large text it grows a little with the text (Avatar caps it), photo or initials alike. */
    size: { xs: 'size-6', sm: 'size-8', md: 'size-10', lg: 'size-14' } satisfies Record<AvatarSize, string>,
    /** A hue's soft fill in place of the accent's, so people are told apart at a glance. */
    hue: { 1: 'bg-hue-1-soft', 2: 'bg-hue-2-soft', 3: 'bg-hue-3-soft', 4: 'bg-hue-4-soft', 5: 'bg-hue-5-soft', 6: 'bg-hue-6-soft', 7: 'bg-hue-7-soft', 8: 'bg-hue-8-soft' } satisfies Record<Hue, string>,
    /** In a group, a 2 px edge in the colour of the surface behind cuts each one out of the next. */
    inGroup: {
      surface: 'border-2 border-surface',
      raised: 'border-2 border-surface-raised',
      sunken: 'border-2 border-surface-sunken',
    } satisfies Record<AvatarGroupSurface, string>,
  },
  image: {
    base: 'absolute inset-0 size-full object-cover',
  },
  initials: {
    base: 'font-semibold text-text',
    size: { xs: 'text-caption', sm: 'text-caption', md: 'text-footnote', lg: 'text-callout' } satisfies Record<AvatarSize, string>,
  },
  /** A dot on the circle's bottom end edge, cut out of the avatar by an edge in the surface's colour. */
  status: {
    base: 'absolute rounded-full border-2',
    // Centred on the circle's edge, a little inside it on the larger sizes, so the edge cuts a visible notch.
    size: { xs: 'size-2 end-0 bottom-0', sm: 'size-2.5 end-0 bottom-0', md: 'size-3 end-0.5 bottom-0.5', lg: 'size-4 end-1 bottom-1' } satisfies Record<AvatarSize, string>,
    edge: { surface: 'border-surface', raised: 'border-surface-raised', sunken: 'border-surface-sunken' } satisfies Record<AvatarGroupSurface, string>,
    tone: {
      // The mark tones: mid-tones made for dots and shapes, where the status colours are made for words.
      online: 'bg-success-mark',
      away: 'bg-warning-mark',
      busy: 'bg-danger-mark',
      // Hollow: a ring of the muted text colour around the surface.
      offline: 'bg-surface inset-ring-2 inset-ring-muted',
    } satisfies Record<AvatarStatus, string>,
  },
  badge: {
    base: 'absolute -end-1.5 -top-1.5',
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
