import { childrenToArray, createContext, cx, useContext, useLocal, useView, type Child } from '@lingxia/view';
import type { Hue } from '../hue';
import { avatarRecipe as r } from './Avatar.recipe';
import { dataOf } from '../internal/data';

export type AvatarSize = 'xs' | 'sm' | 'md' | 'lg';
/** `circle` for a person; `rounded` for a team, a workspace or an app. */
export type AvatarShape = 'circle' | 'rounded';
export type AvatarGroupSurface = 'surface' | 'raised' | 'sunken';
/** `offline` is a hollow dot, so the four are told apart without colour as far as a dot can. */
export type AvatarStatus = 'online' | 'away' | 'busy' | 'offline';

const InGroup = createContext<{ size: AvatarSize; surface: AvatarGroupSurface; shape: AvatarShape } | null>(null);

export interface AvatarProps {
  /** Whose avatar: the accessible name, and the source of the initials. */
  name: string;
  /** A photo; initials show until it loads and if it fails. */
  src?: string;
  /** Overrides the initials taken from the first letters of the first two words of `name`. */
  initials?: string;
  /** The colour behind the initials, one of the theme's eight hues; `hueFor(name)` from `@liulidesign/core/hue` gives each name its own. Without it, the accent's soft step. */
  hue?: Hue;
  /** @default 'md', or the group's size */
  size?: AvatarSize;
  /** @default 'circle', or the group's shape */
  shape?: AvatarShape;
  /** A dot at the bottom end edge: whether the person is around. Give `statusLabel` with it. */
  status?: AvatarStatus;
  /** The status in words, read after the name: "Online", "In a meeting". */
  statusLabel?: string;
  /** Drawn on the top end edge: a Badge with a count. It is read by its own label. */
  badge?: Child;
  /** The surface the avatar sits on; the status dot's edge takes its colour. @default 'surface', or the group's */
  surface?: AvatarGroupSurface;
  /** Hides it from assistive tech when the name is already beside it. @default false */
  isDecorative?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** A person or team as a circle: their photo, or their initials. */
/** Each size's side and its initials' size at the default text size, in points. */
const sides = { xs: 24, sm: 32, md: 40, lg: 56 } satisfies Record<AvatarSize, number>;
const letterSizes = { xs: 12, sm: 12, md: 13, lg: 17 } satisfies Record<AvatarSize, number>;
/** An avatar keeps company with larger text up to this, so at large text it never takes a row's width from the name. */
const maxGrowth = 1.3;

/**
 * The box and the initials at the current text size: the box grows with the text up to `maxGrowth` and stays
 * square, photo or not; the initials stay the same share of it, so one or two always fit.
 */
function useScaled(size: AvatarSize) {
  const scale = Math.max(1, useView().env.textScale ?? 1);
  if (scale === 1) return { box: undefined, letters: undefined };
  const growth = Math.min(scale, maxGrowth);
  const side = Math.round(sides[size] * growth);
  // Interim: the View grows every font by the device text size with no opt-out, so this divides that growth back out; it goes when text can ignore or cap it.
  return { box: { width: side, height: side }, letters: { fontSize: Math.round((letterSizes[size] * growth / scale) * 10) / 10 } };
}

export function Avatar({ name, src, initials, hue, size, shape: ownShape, status, statusLabel, badge, surface, isDecorative = false, className, 'data-testid': testId, ...attributes }: AvatarProps) {
  const group = useContext(InGroup);
  const actual = size ?? group?.size ?? 'md';
  const shape = ownShape ?? group?.shape ?? 'circle';
  const failed = useLocal(false);
  const letters = initials ?? initialsOf(name);
  const showImage = src !== undefined && !failed.value;
  const spoken = statusLabel === undefined ? name : `${name}, ${statusLabel}`;
  const scaled = useScaled(actual);
  return (
    <view
      aria-hidden={isDecorative || undefined}
      data-testid={testId} {...dataOf(attributes)}
      style={scaled.box}
      className={cx(r.root.base, r.root.shape[shape][actual], r.root.size[actual], hue !== undefined && r.root.hue[hue], group && r.root.inGroup[group.surface], className)}
    >
      {/* The initials stay under the photo, so a slow or failed load never shows an empty circle. */}
      <text role={showImage ? undefined : 'img'} aria-label={showImage ? undefined : spoken} aria-hidden={showImage || undefined} style={scaled.letters} className={cx(r.initials.base, r.initials.size[actual])}>{letters}</text>
      {showImage && <image src={src} alt={spoken} onError={() => failed.set(true)} className={cx(r.image.base, r.root.shape[shape][actual])} />}
      {status !== undefined && <view aria-hidden className={cx(r.status.base, r.status.size[actual], r.status.edge[surface ?? group?.surface ?? 'surface'], r.status.tone[status])} />}
      {badge !== undefined && <view className={r.badge.base}>{badge}</view>}
    </view>
  );
}

function initialsOf(name: string): string {
  return name.trim().split(/\s+/).slice(0, 2).map(word => Array.from(word)[0] ?? '').join('').toUpperCase();
}

export interface AvatarGroupProps {
  /** Names the group for assistive tech: "Members". */
  label: string;
  /** The avatars, in order; past `max` they fold into one "+n". */
  children: Child;
  /** How many to show before the rest fold into one count. */
  max?: number;
  /** The count's accessible name, written by the app: "4 more people". */
  overflowLabel?: (count: number) => string;
  /** @default 'md' */
  size?: AvatarSize;
  /** The shape of every avatar in the group, and of the count. @default 'circle' */
  shape?: AvatarShape;
  /** The surface the group sits on; each avatar's edge takes its colour. @default 'surface' */
  surface?: AvatarGroupSurface;
  className?: string;
  'data-testid'?: string;
}

/** Avatars overlapping in a row, with the rest as a count. */
export function AvatarGroup({ label, children, max, overflowLabel, size = 'md', shape = 'circle', surface = 'surface', className, 'data-testid': testId, ...attributes }: AvatarGroupProps) {
  const all = childrenToArray(children);
  const shown = max === undefined ? all : all.slice(0, max);
  const rest = all.length - shown.length;
  const scaled = useScaled(size);
  return (
    <InGroup.Provider value={{ size, surface, shape }}>
      <view role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.group.base, className)}>
        {shown.map((child, index) => (
          <view key={String(index)} className={cx(index > 0 && r.overlap.size[size], r.slot[surface], r.root.shape[shape][size])}>{child}</view>
        ))}
        {rest > 0 && (
          <view className={cx(r.overlap.size[size], r.slot[surface], r.root.shape[shape][size])}>
            <view data-testid={testId && `${testId}-overflow`} style={scaled.box} className={cx(r.root.base, r.root.shape[shape][size], r.root.size[size], r.root.inGroup[surface])}>
              <text aria-label={overflowLabel?.(rest)} style={scaled.letters} className={cx(r.initials.base, r.initials.size[size], r.overflow.text)}>{`+${rest}`}</text>
            </view>
          </view>
        )}
      </view>
    </InGroup.Provider>
  );
}
