<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Switch

A setting that turns on or off the moment it flips. Status: preview.

```ts
import { Switch } from '@liulidesign/core/Switch';
```

Switch turns one setting on or off and applies it at once, with no Save. The label and the track are one press target; help text below is read as the description.

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
  return (
    <view className="-m-5 divide-y divide-text/8">
      <Item label="Sound" endContent={<text className="text-body">Chime</text>} onClick={() => {}} hasChevron />
      <Switch bind={alerts} label="Mentions" variant="row" data-testid="mentions" />
      <Switch bind={digest} label="Weekly digest" variant="row" data-testid="digest" />
    </view>
  );
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
| `isDisabled` | `boolean` | `false` |  |
| `variant` | `SwitchVariant` | `'standalone'` | `row` sits in a list group among `Item` rows, with the row's padding and height. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the switch. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| control | `<switch>` | yes | The switch primitive: the label at the start, the track and thumb at the end, and the accessible node. |
| description | `<text>` | no | Help text under the label, read as the description. |

## Accessibility

- Pattern: switch.
- Name: label.
- Description: description.
- States: checked (from value or bind); disabled (from isDisabled).
- Space: Flips it..
- Press target: md 32 px.
- On and off differ in the thumb's side as well as the track colour, never in colour alone.
- The standalone switch is 32 px tall; the row variant gives it a 48 px row.

| Contrast | Pair | Minimum | Proven by the theme pack |
| --- | --- | --- | --- |
| track, on | `accent` on `surface` | 3:1 | yes |
| track, off | `line-strong` on `surface` | 3:1 | yes |
| label | `text` on `surface` | 4.5:1 | yes |

## Guidance

- Do: Name the setting, not the action: "Wi-Fi", not "Turn on Wi-Fi".
- Do: Use a switch only when the change applies at once.
- Don't: Use a switch on a form with a Save button; use a CheckboxInput.
- Don't: Put the state in the label ("Wi-Fi on"); the switch shows it.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Liuli's own track and thumb: a thumb with a hairline and contact shadow, a smaller track for dense rows. | switch track and thumb styling (size, fill, border, shadow) on the primitive | The primitive's 52 by 32 track and plain white thumb, at one size. |
| Help text that toggles the switch when tapped. | a switch that takes element children, as a checkbox does | Only the label row is the press target; the help text sits outside it. |

Related: CheckboxInput, Item.
