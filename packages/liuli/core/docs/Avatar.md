<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Avatar

A person or team as a circle: their picture, or their initials. Status: preview.

```ts
import { Avatar, AvatarGroup } from '@liulidesign/core/Avatar';
```

Avatar shows who: a picture when there is one, initials when there is none or it fails to load. AvatarGroup overlaps several in a row and folds the rest into a count.

## Examples

### A picture, initials, and a failed picture

```tsx
import { Avatar } from '@liulidesign/core/Avatar';

const photo = (n: number) => `public/avatars/avatar-${n}.png`;

/** A photo, and initials where there is none or it fails to load. */
export function People() {
  return (
    <view className="flex-row items-center gap-3">
      <Avatar name="Ana Lima" src={photo(1)} data-testid="ana" />
      <Avatar name="Kai Chen" data-testid="kai" />
      <Avatar name="Mira Patel" src="public/avatars/missing.png" data-testid="mira" />
    </view>
  );
}
```

### Sizes

```tsx
import { Avatar } from '@liulidesign/core/Avatar';

export function Sizes() {
  return (
    <view className="flex-row items-center gap-3">
      <Avatar name="Ana Lima" size="xs" />
      <Avatar name="Ana Lima" size="sm" />
      <Avatar name="Ana Lima" size="md" />
      <Avatar name="Ana Lima" size="lg" />
    </view>
  );
}
```

### A group with a count

```tsx
import { Avatar, AvatarGroup } from '@liulidesign/core/Avatar';

const photo = (n: number) => `public/avatars/avatar-${n}.png`;

/** Overlapping, with the rest folded into a count. */
export function Group() {
  return (
    <AvatarGroup label="Editors" max={4} overflowLabel={count => `${count} more editors`} data-testid="editors">
      <Avatar name="Ana Lima" src={photo(1)} />
      <Avatar name="Kai Chen" />
      <Avatar name="Mira Patel" src={photo(2)} />
      <Avatar name="Tomás Ruiz" />
      <Avatar name="Lena Vogel" />
      <Avatar name="Sam Okafor" />
    </AvatarGroup>
  );
}
```

### Beside the name

```tsx
import { Avatar } from '@liulidesign/core/Avatar';

/** Beside the name it stands for, so it is decorative. */
export function WithName() {
  return (
    <view className="flex-row items-center gap-3">
      <Avatar name="Kai Chen" isDecorative />
      <view className="gap-0.5">
        <text className="text-body font-medium text-text">Kai Chen</text>
        <text className="text-footnote text-muted">Edited 2 minutes ago</text>
      </view>
    </view>
  );
}
```

### A status dot

```tsx
import { Avatar } from '@liulidesign/core/Avatar';

const photo = (n: number) => `public/avatars/avatar-${n}.png`;

/** Whether each person is around, as a dot on the avatar and in words for a screen reader. */
export function Presence() {
  return (
    <view className="flex-row items-center gap-4">
      <Avatar name="Ana Lima" src={photo(1)} status="online" statusLabel="Online" data-testid="online" />
      <Avatar name="Kai Chen" status="away" statusLabel="Away" data-testid="away" />
      <Avatar name="Mira Patel" status="busy" statusLabel="Do not disturb" data-testid="busy" />
      <Avatar name="Sam Ortiz" status="offline" statusLabel="Offline" data-testid="offline" />
      <Avatar name="Design team" size="lg" status="online" statusLabel="3 online" />
    </view>
  );
}
```

### A count on the avatar

```tsx
import { Badge } from '@liulidesign/core/Badge';
import { Avatar } from '@liulidesign/core/Avatar';

/** A count on the avatar: unread messages from this person. */
export function Unread() {
  return <Avatar name="Kai Chen" badge={<Badge label="3" tone="danger" emphasis="solid" size="sm" />} data-testid="unread" />;
}
```

### A rounded square for a team

```tsx
import { Avatar } from '@liulidesign/core/Avatar';

export function Teams() {
  return (
    <view className="gap-3">
      <view className="flex-row items-center gap-3">
        <Avatar name="Design Systems" shape="rounded" isDecorative data-testid="team" />
        <view className="gap-0.5">
          <text className="text-body font-medium text-text">Design Systems</text>
          <text className="text-footnote text-muted">Team · 12 members</text>
        </view>
      </view>
      <view className="flex-row items-center gap-3">
        <Avatar name="Ana Lima" isDecorative />
        <view className="gap-0.5">
          <text className="text-body font-medium text-text">Ana Lima</text>
          <text className="text-footnote text-muted">Owner</text>
        </view>
      </view>
    </view>
  );
}
```

### A hue for each person

```tsx
import { hueFor } from '../hue';
import { Avatar } from '@liulidesign/core/Avatar';

/** A rounded square for what is not a person: a team, a workspace. */
/** Each name gets its own hue, the same one every time, so a list of initials is not a column of one colour. */
export function Hues() {
  const people = ['Ana Lima', 'Kai Chen', 'Mira Patel', 'Tomas Reyes', 'Yuki Sato', 'Noor Haddad'];
  return (
    <view className="flex-row flex-wrap gap-3">
      {people.map(name => <Avatar key={name} name={name} hue={hueFor(name)} data-testid={`hue-${name.split(' ')[0].toLowerCase()}`} />)}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `name` (required) | `string` |  | Whose avatar: the accessible name, and the source of the initials. |
| `src` | `string` |  | A photo; initials show until it loads and if it fails. |
| `initials` | `string` |  | Overrides the initials taken from the first letters of the first two words of `name`. |
| `hue` | `Hue` |  | The colour behind the initials, one of the theme's eight hues; `hueFor(name)` from `@liulidesign/core/hue` gives each name its own. Without it, the accent's soft step. |
| `size` | `AvatarSize` | `'md', or the group's size` |  |
| `shape` | `AvatarShape` | `'circle', or the group's shape` |  |
| `status` | `AvatarStatus` |  | A dot at the bottom end edge: whether the person is around. Give `statusLabel` with it. |
| `statusLabel` | `string` |  | The status in words, read after the name: "Online", "In a meeting". |
| `badge` | `Child` |  | Drawn on the top end edge: a Badge with a count. It is read by its own label. |
| `surface` | `AvatarGroupSurface` | `'surface', or the group's` | The surface the avatar sits on; the status dot's edge takes its colour. |
| `isDecorative` | `boolean` | `false` | Hides it from assistive tech when the name is already beside it. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The circle, on the accent's soft step. |
| initials | `<text>` | yes | Taken from `name`, or `initials`; under the picture, so a slow load never shows an empty circle. |
| image | `<image>` | no | The picture from `src`, named by `name`; dropped if it fails. |
| group | `<view>` | no | AvatarGroup: a named group, each one cut out of the next by a ring in the surface colour. |
| overflow | `<text>` | no | The "+n" for avatars past `max`, named by `overflowLabel`. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `avatarRecipe` from `@liulidesign/core/Avatar/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-callout`.
- Colour roles it paints with, set by the theme pack: `accent-soft`, `danger-mark`, `hue-1-soft`, `hue-2-soft`, `hue-3-soft`, `hue-4-soft`, `hue-5-soft`, `hue-6-soft`, `hue-7-soft`, `hue-8-soft`, `line`, `on-accent-soft`, `success-mark`, `surface`, `surface-raised`, `surface-sunken`, `text`, `warning-mark`.

## Accessibility

- Pattern: none.
- Name: name.
- Description: none.
- Read as a picture of the person, by name, whether it shows the photo or the initials. Set `isDecorative` when the name is already written beside it.
- The group is read by its label, then each name; the count by the app's own words.
- A status dot is drawn only: `statusLabel` is read after the name, "Ana Lima, Online". The four statuses differ by colour, and offline also by being hollow, so always pass the label.
- A `badge` is read by its own label; say what the number counts in a label beside the avatar or in the row it sits in.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| initials | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.44:1 | 9.49:1 |
| hue 1, initials on its fill | `text` on `hue-1-soft` | 4.5:1 | 14.23:1 | 11.25:1 |
| hue 2, initials on its fill | `text` on `hue-2-soft` | 4.5:1 | 14.45:1 | 11.01:1 |
| hue 3, initials on its fill | `text` on `hue-3-soft` | 4.5:1 | 14.24:1 | 11.37:1 |
| hue 4, initials on its fill | `text` on `hue-4-soft` | 4.5:1 | 14.32:1 | 11.18:1 |
| hue 5, initials on its fill | `text` on `hue-5-soft` | 4.5:1 | 14.13:1 | 11.41:1 |
| hue 6, initials on its fill | `text` on `hue-6-soft` | 4.5:1 | 14.10:1 | 11.41:1 |
| hue 7, initials on its fill | `text` on `hue-7-soft` | 4.5:1 | 14.32:1 | 11.05:1 |
| hue 8, initials on its fill | `text` on `hue-8-soft` | 4.5:1 | 14.31:1 | 11.20:1 |
| online dot | `success-mark` on `surface` | 3:1 | 3.61:1 | 6.91:1 |
| away dot | `warning-mark` on `surface` | 3:1 | 3.64:1 | 8.03:1 |
| busy dot | `danger-mark` on `surface` | 3:1 | 3.91:1 | 5.19:1 |
| online dot, in a sunken card | `success-mark` on `surface-sunken` | 3:1 | 3.19:1 | 7.75:1 |
| away dot, in a sunken card | `warning-mark` on `surface-sunken` | 3:1 | 3.22:1 | 9.00:1 |
| busy dot, in a sunken card | `danger-mark` on `surface-sunken` | 3:1 | 3.45:1 | 5.82:1 |
| offline dot's ring | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Mark an avatar decorative when the name sits next to it, so the name is not read twice.
- Do: Cap a group with `max` and say what the count means in `overflowLabel`.
- Don't: Use an avatar as the only way to tell people apart in a list; show names too.
- Don't: Make an avatar a button on its own; put it inside an Item or Button that says what it opens.

Related: Item, Badge.
