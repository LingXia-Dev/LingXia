<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Switch

A setting that turns on or off the moment it flips. Status: preview.

```ts
import { Switch } from '@liulidesign/core/Switch';
```

Switch turns one setting on or off and applies it at once, with no Save. The label, its help text and the track are one press target; the help text is read as the description.

## Examples

### With help text

```tsx
import { useLocal } from '@lingxia/view';
import { Switch } from '@liulidesign/core/Switch';

/** A setting that applies the moment it flips. */
export function Basic() {
  const sync = useLocal('sync', true);
  return <Switch bind={sync} label="Sync over mobile data" description="Photos wait for Wi-Fi either way." data-testid="sync" />;
}
```

### As list rows

```tsx
import { useLocal } from '@lingxia/view';
import { Item } from '@liulidesign/core/Item';
import { Switch } from '@liulidesign/core/Switch';

/** Among list rows: the row's padding and height. The stage's own padding is undone so the rows meet its edges. */
export function InList() {
  const alerts = useLocal('alerts', true);
  const digest = useLocal('digest', false);
  const sounds = ['Chime', 'Bell', 'None'];
  const sound = useLocal('sound', 0);
  return (
    <view className="-m-5 divide-y divide-text/8">
      {/* An app opens a list of sounds here; the row steps through them so the press shows. */}
      <Item label="Sound" endContent={<text className="text-body">{sounds[sound.value]}</text>} onClick={() => sound.set((sound.value + 1) % sounds.length)} hasChevron />
      <Switch bind={alerts} label="Mentions" variant="row" data-testid="mentions" />
      <Switch bind={digest} label="Weekly digest" variant="row" data-testid="digest" />
    </view>
  );
}
```

### Small and medium

```tsx
import { Held } from '../example';
import { Switch } from '@liulidesign/core/Switch';

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value={true}>{(on, set) => <Switch value={on} onChange={set} label="Small, for dense rows" size="sm" />}</Held>
      <Held value={true}>{(on, set) => <Switch value={on} onChange={set} label="Medium" />}</Held>
    </view>
  );
}
```

### With its label hidden, in a row that names it

```tsx
import { useLocal } from '@lingxia/view';
import { Switch } from '@liulidesign/core/Switch';

/** In a row that names the setting already: the label is the switch's name, not drawn. */
export function LabelHidden() {
  const pinned = useLocal('pinned', false);
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow text-body text-text">Keep this chat at the top</text>
      <Switch bind={pinned} label="Pin chat" isLabelHidden data-testid="pin" />
    </view>
  );
}
```

### Waiting for the app to apply it

```tsx
import { unused } from '../example';
import { Switch } from '@liulidesign/core/Switch';

/** While the app applies a change that takes a moment, the switch waits and says so. */
export function Applying() {
  return <Switch value onChange={unused} label="Back up over Wi-Fi" description="Turning on: checking your storage." isLoading data-testid="applying" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<boolean>` |  | A View-local cell the switch reads and writes; use it or `value` with `onChange`. |
| `value` | `boolean` |  | The app's value; pass `onChange` with it. |
| `onChange` | `((value: boolean) => void)` |  | Runs with the new value; the app passes it back as `value`. |
| `label` (required) | `string` |  | The visible label and the switch's name. Say what is on, not the action: "Wi-Fi", "Sync over mobile data". |
| `description` | `string` |  | Help text under the label, read as the switch's description. |
| `isLabelHidden` | `boolean` | `false` | Keeps `label` as the name and draws only the track, for a switch whose row already says what it sets. Help text is not drawn either. |
| `isDisabled` | `boolean` | `false` |  |
| `isLoading` | `boolean` | `false` | A spinner before the track while the app applies the change; the switch reports busy and takes no press until it settles. |
| `size` | `SwitchSize` | `'md'` | `sm` for dense rows. |
| `variant` | `SwitchVariant` | `'standalone'` | `row` sits in a list group among `Item` rows, with the row's padding and height. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the switch. |

- `bind` | `value` + `onChange`: props from one side only.

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| control | `<switch>` | yes | The switch primitive: its content at the start, the track and thumb at the end, and the accessible node. |
| label | `<text>` | yes | The visible label, also the name. |
| description | `<text>` | no | Help text under the label, inside the press target, read as the description. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `switchRecipe` from `@liulidesign/core/Switch/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--shadow-contact`.
- Colour roles it paints with, set by the theme pack: `accent-solid`, `focus`, `knob`, `line-strong`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: switch.
- Name: label.
- Description: description.
- States: checked (from value or bind); disabled (from isDisabled).
- Space: Flips it..
- Press target: sm 32 px, md 32 px.
- On and off differ in the thumb's side as well as the track colour, never in colour alone.
- The standalone switch is 32 px tall at both sizes; the row variant gives it a 48 px row.
- The off track is decorative and has no contrast minimum: off is carried by the thumb at the start and the missing accent.
- Disabled and on, the track keeps its accent at reduced strength, so the state still reads.
- The thumb is the `knob` role, white in both appearances; the on track is `accent-solid`, the fill made to carry white.
- Keyboard focus rings a standalone switch; on a row it rings the row and its help text.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| track, on, against the surface | `surface` on `accent-solid` | 3:1 | 5.62:1 | 3.50:1 |
| thumb against the track, on | `knob` on `accent-solid` | 3:1 | 5.62:1 | 4.80:1 |
| keyboard focus ring | `focus` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the setting, not the action: "Wi-Fi", not "Turn on Wi-Fi".
- Do: Use a switch only when the change applies at once.
- Don't: Use a switch on a form with a Save button; use a CheckboxInput.
- Don't: Put the state in the label ("Wi-Fi on"); the switch shows it.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Keyboard focus shown only by the thumb ring or the row ring. | the switch primitive's built-in focus ring giving way to authored focus styles | The built-in ring also draws around the track. |

Related: CheckboxInput, Item.
