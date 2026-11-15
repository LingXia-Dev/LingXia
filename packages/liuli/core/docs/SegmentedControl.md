<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# SegmentedControl

One value from a few, as a row of segments with a thumb on the chosen one. Status: preview.

```ts
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
```

SegmentedControl sets one value from two to five short choices that fit a row: a unit, a theme, a sort order. It is a radio group drawn as equal segments, with a raised thumb that slides to the chosen one. It sets a value; to switch what a panel shows, use Tabs.

## Examples

### A unit for the numbers below it

```tsx
import { useLocal } from '@lingxia/view';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';

/** A unit for the numbers below it, bound to a View-local cell. */
export function Units() {
  const unit = useLocal('unit', 'km');
  const distance = unit.value === 'km' ? '12.4 km' : '7.7 mi';
  return (
    <view className="gap-3">
      <SegmentedControl
        label="Units"
        options={[{ value: 'km', label: 'Kilometres' }, { value: 'mi', label: 'Miles' }]}
        bind={unit}
        data-testid="units"
      />
      <view className="gap-0.5">
        <text className="text-footnote text-muted">This week</text>
        <text className="text-title font-semibold text-text tabular-nums" data-testid="distance">{distance}</text>
      </view>
    </view>
  );
}
```

### A setting row

```tsx
import { useLocal } from '@lingxia/view';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';

const themes = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

/** A setting row: the control sits at the end, sized to its segments. */
export function Appearance() {
  const theme = useLocal('theme', 'system');
  return (
    <view className="flex-row items-center gap-4">
      <text className="grow basis-0 text-body text-text">Appearance</text>
      <SegmentedControl label="Appearance" options={themes} value={theme.value} onChange={next => theme.set(next)} size="sm" className="w-56" data-testid="appearance" />
    </view>
  );
}
```

### Sizes

```tsx
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';

const themes = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

export function Sizes() {
  return (
    <view className="gap-3">
      <SegmentedControl label="Small" options={themes} value="light" onChange={() => {}} size="sm" />
      <SegmentedControl label="Medium" options={themes} value="light" onChange={() => {}} />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string>` |  | A View-local cell the control reads and writes; use it or `value` with `onChange`. |
| `value` | `string` |  | The chosen segment's value. |
| `onChange` | `((value: string) => void)` |  | Runs with the segment chosen by a tap or by arrowing onto it. |
| `label` (required) | `string` |  | Names the group for assistive tech: "Units", "Sort". |
| `options` (required) | `readonly SegmentedOption[]` |  | Two to five choices, in order; they share the row equally. |
| `size` | `SegmentedControlSize` | `'md'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group; each segment gets `<testid>-<value>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The sunken track and the radio group, named by `label`. |
| track | `<view>` | yes | Holds the thumb; mirrors in right-to-left so it slides toward the end. |
| thumb | `<view>` | yes | The raised box, one segment wide, under the chosen segment. |
| segment | `<button>` | yes | A radio per option, named by its label. |
| label | `<text>` | yes | The option's text, cut short if its share of the row is too narrow. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `segmentedControlRecipe` from `@liulidesign/core/SegmentedControl/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--shadow-contact`, `--radius-control`, `--radius-segment`.
- Colour roles it paints with, set by the theme pack: `focus`, `surface-raised`, `surface-sunken`, `text`, `text-muted`.

## Accessibility

- Pattern: radioGroup.
- Name: label.
- Description: none.
- States: checked (from value, or bind); disabled (from options[].isDisabled).
- Tab: Into the row, on the chosen segment; again, out of it..
- Left, Right: The previous or next segment, chosen on arrival..
- Press target: sm 28 px, md 36 px.
- The chosen segment is in the raised thumb and the label's colour together, and is read as checked.
- Segments are 28 and 36 px tall, below the 44 pt touch minimum; keep it to pointer layouts and compact rows with room around them.
- The thumb's edge is decorative and has no contrast minimum: the chosen segment is carried by its label and the raised thumb.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| chosen label | `text` on `surface-raised` | 4.5:1 | 16.26:1 | 11.83:1 |
| other labels | `text-muted` on `surface-sunken` | 4.5:1 | 5.20:1 | 7.45:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep to two to five labels short enough to fit their share of the row.
- Do: Apply the choice at once, where people can see what it changed.
- Don't: Use it to switch between panels of content; that is Tabs.
- Don't: Use it for more than five choices or long labels; use a Select or a RadioGroup.
- Don't: Use it for an action; each segment is a value, not a command.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Segments as wide as their labels, with the thumb following. | a way to read a laid-out box's position and width from View, or a shared-element layout motion between siblings | Segments share the row equally, so the thumb moves by a segment's width. |
| Icon-only segments. | an icon set for Liuli | Segments carry a text label. |

Related: Tabs, RadioGroup, ToggleButton.
