<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# NumberInput

A number typed or nudged by steps, kept between a minimum and a maximum. Status: preview.

```ts
import { NumberInput } from '@liulidesign/core/NumberInput';
```

NumberInput holds one number: a count of guests, a weight, a price. People can type it, or nudge it with the two steppers at the end of the box. It carries a number, or `null` while the field is empty, so the app never parses text. A unit or a currency sign can sit inside the box, and it takes Field's label, help text and status.

## Examples

### A count with steppers

```tsx
import { useLocal } from '@lingxia/view';
import { NumberInput } from '@liulidesign/core/NumberInput';

/** A count people nudge more often than they type. */
export function Guests() {
  const guests = useLocal<number | null>('guests', 2);
  return (
    <NumberInput
      label="Guests"
      description="Up to 12 at one table."
      bind={guests}
      min={1}
      max={12}
      decrementLabel="Fewer guests"
      incrementLabel="More guests"
      data-testid="guests"
    />
  );
}
```

### A unit, and steps of a tenth

```tsx
import { useLocal } from '@lingxia/view';
import { NumberInput } from '@liulidesign/core/NumberInput';

/** A unit after the number, and steps of a tenth. */
export function Weight() {
  const weight = useLocal<number | null>('weight', 71.5);
  return (
    <NumberInput label="Weight" bind={weight} min={0} step={0.1} suffix="kg" decrementLabel="Lighter" incrementLabel="Heavier" data-testid="weight" />
  );
}
```

### A typed number, without steppers

```tsx
import { useLocal } from '@lingxia/view';
import { NumberInput } from '@liulidesign/core/NumberInput';

/** A number people type: a price, with its currency, and no steppers. */
export function Price() {
  const price = useLocal<number | null>('price', null);
  return (
    <NumberInput
      label="Price"
      bind={price}
      min={0}
      step={0.01}
      prefix="$"
      placeholder="0.00"
      hasSteppers={false}
      optionalLabel="optional"
      decrementLabel="Lower"
      incrementLabel="Higher"
      data-testid="price"
    />
  );
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { NumberInput } from '@liulidesign/core/NumberInput';

export function Sizes() {
  const value = useLocal<number | null>('sized', 4);
  return (
    <view className="gap-3">
      <NumberInput label="Small" size="sm" bind={value} decrementLabel="Less" incrementLabel="More" data-testid="small" />
      <NumberInput label="Medium" bind={value} decrementLabel="Less" incrementLabel="More" />
      <NumberInput label="Large" size="lg" bind={value} decrementLabel="Less" incrementLabel="More" data-testid="large" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible label and the control's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Hides the visible label; the control keeps it as its name. |
| `description` | `string` |  | Help text under the control, read as the control's description. |
| `status` | `FieldStatus` |  | An `error` status marks the control invalid; its message is read as the control's description. |
| `isRequired` | `boolean` | `false` | Marks the label with an asterisk and the control as required. Validation is the app's. |
| `hasRequiredMark` | `boolean` | `true` | Draws the asterisk on a required label; turn it off on a form where every field is required. |
| `optionalLabel` | `string` |  | A word after the label for a field that may be left empty, as the app writes it: "optional". Read before the help text. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `bind` | `Local<number \| null>` |  | A View-local cell the control reads and writes; `null` while the field is empty. Use it or `value` with `onChange`. |
| `value` | `number \| null` |  | The number, or `null` while the field is empty. |
| `onChange` | `((value: number \| null) => void)` |  | Runs with the new number, or `null` when the field is emptied. |
| `min` | `number` |  | The lowest number the steppers reach and the field keeps. |
| `max` | `number` |  | The highest number the steppers reach and the field keeps. |
| `step` | `number` | `1` | How much a stepper adds or takes away; its decimals are the number's. |
| `decrementLabel` (required) | `string` |  | The minus stepper's accessible name: "Fewer guests". |
| `incrementLabel` (required) | `string` |  | The plus stepper's accessible name: "More guests". |
| `hasSteppers` | `boolean` | `true` | Leaves the steppers out, for a number people type and never nudge. |
| `prefix` | `string` |  | Fixed text before the number: a currency sign. Read with the description. |
| `suffix` | `string` |  | Fixed text after the number: a unit. Read with the description. |
| `placeholder` | `string` |  |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the editor; the steppers are `<id>-decrement` and `<id>-increment`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | TextInput's box, holding the number, any fixed text and the steppers. |
| input | `<input>` | yes | The editor, in tabular figures; the element assistive tech names by `label`. |
| adornment | `<text>` | no | `prefix` and `suffix`: fixed text beside the number, read with the description. |
| steppers | `<view>` | no | Minus and plus at the end of the box, behind a hairline. |
| stepper | `<button>` | no | One stepper: a square as tall as the box, named by `decrementLabel` or `incrementLabel`, disabled at its end of the range. |

## Accessibility

- Pattern: textbox.
- Name: label.
- Description: prefix, then suffix, then optionalLabel, then description, then status message.
- States: invalid (from status.type === "error"); required (from isRequired); disabled (from isDisabled).
- Tab: Moves from the number to the minus stepper, then the plus stepper.
- Enter or Space: On a stepper: one step down or up.
- Press target: sm 32 px, md 40 px, lg 48 px.
- A number editor named by `label`; the unit, then the help text and status, are read as its description.
- Each stepper is a button with its own name from the app, so a screen reader hears what a press changes: "More guests".
- A stepper at its end of the range is disabled, and the field keeps typed numbers within `min` and `max` when it loses focus.
- The hairlines beside the steppers are decoration; each stepper is told apart by its mark.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| number | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| unit, placeholder and stepper marks | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| field edge | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| error edge | `danger` on `surface` | 3:1 | 5.21:1 | 5.42:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use steppers for small counts people adjust by one or two; leave them out with `hasSteppers={false}` for numbers people type.
- Do: Set `min`, `max` and `step` to what the number can be; `step` also sets how many decimals a stepper keeps.
- Do: Put the unit in `suffix` or the currency in `prefix`, not in the label.
- Do: Name the steppers for what changes: "Fewer guests", "More guests".
- Don't: Use it for numbers that are not amounts: a phone number, a postcode, a card number. Those are text.
- Don't: Use it for a value picked by feel along a range; that is a Slider.
- Don't: Reject a number while it is being typed; show a status once it is wrong.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| The number shown formatted (grouped digits, a fixed number of decimals) while the field is not being edited. | focus and blur events on an editor | The field shows the digits as typed; a unit or currency sits beside them. |
| Up and Down arrow keys stepping the number from the editor, and the spin-button role. | key events from an editor and a spinbutton role | The steppers are two buttons after the editor in the Tab order. |
| A stepper that repeats while held. | a press-and-hold event | One step per press. |

Related: TextInput, Slider.
