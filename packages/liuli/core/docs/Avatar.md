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

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `name` (required) | `string` |  | Whose avatar: the accessible name, and the source of the initials. |
| `src` | `string` |  | A photo; initials show until it loads and if it fails. |
| `initials` | `string` |  | Overrides the initials taken from the first letters of the first two words of `name`. |
| `size` | `AvatarSize` | `'md', or the group's size` |  |
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

## Accessibility

- Pattern: none.
- Name: name.
- Description: none.
- Read as a picture of the person, by name, whether it shows the photo or the initials. Set `isDecorative` when the name is already written beside it.
- The group is read by its label, then each name; the count by the app's own words.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| initials | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.44:1 | 9.49:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Mark an avatar decorative when the name sits next to it, so the name is not read twice.
- Do: Cap a group with `max` and say what the count means in `overflowLabel`.
- Don't: Use an avatar as the only way to tell people apart in a list; show names too.
- Don't: Make an avatar a button on its own; put it inside an Item or Button that says what it opens.

Related: Item, Badge.
