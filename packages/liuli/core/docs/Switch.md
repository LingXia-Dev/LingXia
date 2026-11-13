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
  return (
    <view className="-m-5 divide-y divide-text/8">
      <Item label="Sound" endContent={<text className="text-body">Chime</text>} onClick={() => {}} hasChevron />
      <Switch bind={alerts} label="Mentions" variant="row" data-testid="mentions" />
      <Switch bind={digest} label="Weekly digest" variant="row" data-testid="digest" />
    </view>
  );
}
```

### Small and medium

```tsx
import { Switch } from '@liulidesign/core/Switch';

export function Sizes() {
  return (
    <view className="gap-3">
      <Switch value onChange={() => {}} label="Small, for dense rows" size="sm" />
      <Switch value onChange={() => {}} label="Medium" />
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
| `size` | `SwitchSize` | `'md'` | `sm` for dense rows. |
| `variant` | `SwitchVariant` | `'standalone'` | `row` sits in a list group among `Item` rows, with the row's padding and height. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the switch. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| control | `<switch>` | yes | The switch primitive: its content at the start, the track and thumb at the end, and the accessible node. |
| label | `<text>` | yes | The visible label, also the name. |
| description | `<text>` | no | Help text under the label, inside the press target, read as the description. |

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
- Keyboard focus rings a standalone switch; on a row it is a fill across the row and its help text, as on an Item.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| track, on | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| thumb against the track, on | `surface-raised` on `accent` | 3:1 | 5.62:1 | 5.28:1 |
| keyboard focus ring | `focus` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the setting, not the action: "Wi-Fi", not "Turn on Wi-Fi".
- Do: Use a switch only when the change applies at once.
- Don't: Use a switch on a form with a Save button; use a CheckboxInput.
- Don't: Put the state in the label ("Wi-Fi on"); the switch shows it.

Related: CheckboxInput, Item.
