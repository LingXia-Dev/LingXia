import { childrenToArray, createContext, cx, useContext, useLocal, type Child } from '@lingxia/view';
import { avatarRecipe as r } from './Avatar.recipe';

export type AvatarSize = 'xs' | 'sm' | 'md' | 'lg';
export type AvatarGroupSurface = 'surface' | 'raised' | 'sunken';

const InGroup = createContext<{ size: AvatarSize; surface: AvatarGroupSurface } | null>(null);

export interface AvatarProps {
  /** Whose avatar: the accessible name, and the source of the initials. */
  name: string;
  /** A photo; initials show until it loads and if it fails. */
  src?: string;
  /** Overrides the initials taken from the first letters of the first two words of `name`. */
  initials?: string;
  /** @default 'md', or the group's size */
  size?: AvatarSize;
  /** Hides it from assistive tech when the name is already beside it. @default false */
  isDecorative?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** A person or team as a circle: their photo, or their initials. */
export function Avatar({ name, src, initials, size, isDecorative = false, className, 'data-testid': testId }: AvatarProps) {
  const group = useContext(InGroup);
  const actual = size ?? group?.size ?? 'md';
  const failed = useLocal(false);
  const letters = initials ?? initialsOf(name);
  const showImage = src !== undefined && !failed.value;
  return (
    <view
      aria-hidden={isDecorative || undefined}
      data-testid={testId}
      className={cx(r.root.base, r.root.size[actual], group && r.root.inGroup[group.surface], className)}
    >
      {/* The initials stay under the photo, so a slow or failed load never shows an empty circle. */}
      <text role={showImage ? undefined : 'img'} aria-label={showImage ? undefined : name} aria-hidden={showImage || undefined} className={cx(r.initials.base, r.initials.size[actual])}>{letters}</text>
      {showImage && <image src={src} alt={name} onError={() => failed.set(true)} className={r.image.base} />}
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
  /** The surface the group sits on; each avatar's edge takes its colour. @default 'surface' */
  surface?: AvatarGroupSurface;
  className?: string;
  'data-testid'?: string;
}

/** Avatars overlapping in a row, with the rest as a count. */
export function AvatarGroup({ label, children, max, overflowLabel, size = 'md', surface = 'surface', className, 'data-testid': testId }: AvatarGroupProps) {
  const all = childrenToArray(children);
  const shown = max === undefined ? all : all.slice(0, max);
  const rest = all.length - shown.length;
  return (
    <InGroup.Provider value={{ size, surface }}>
      <view role="group" aria-label={label} data-testid={testId} className={cx(r.group.base, className)}>
        {shown.map((child, index) => (
          <view key={String(index)} className={cx(index > 0 && r.overlap.size[size])}>{child}</view>
        ))}
        {rest > 0 && (
          <view className={cx(r.root.base, r.root.size[size], r.root.inGroup[surface], r.overflow.base, r.overlap.size[size])}>
            <text aria-label={overflowLabel?.(rest)} className={cx(r.initials.base, r.initials.size[size], r.overflow.text)}>{`+${rest}`}</text>
          </view>
        )}
      </view>
    </InGroup.Provider>
  );
}
