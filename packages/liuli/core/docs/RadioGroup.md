<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# RadioGroup

One choice from a short list, every option in view. Status: preview.

```ts
import { RadioGroup } from '@liulidesign/core/RadioGroup';
```

RadioGroup picks one option from two to six, all visible at once, as a plain list or as cards. It is a Field: the label names the group, and help text and status describe it.

## Examples

### Options with a second line

```tsx
import { useLocal } from '@lingxia/view';
import { RadioGroup } from '@liulidesign/core/RadioGroup';

const deliveries = [
  { value: 'standard', label: 'Standard', description: '3 to 5 working days, free' },
  { value: 'express', label: 'Express', description: 'Next working day, €6' },
  { value: 'pickup', label: 'Pick up in store', description: 'Ready in two hours' },
];

/** Options with a second line each. */
export function Delivery() {
  const delivery = useLocal<string | null>('delivery', 'standard');
  return <RadioGroup bind={delivery} label="Delivery" items={deliveries} data-testid="delivery" />;
}
```

### Cards

```tsx
import { useLocal } from '@lingxia/view';
import { RadioGroup } from '@liulidesign/core/RadioGroup';

const plans = [
  { value: 'free', label: 'Free', description: 'Up to 3 notebooks, on one device' },
  { value: 'plus', label: 'Plus · €4 a month', description: 'Unlimited notebooks, sync across devices' },
  { value: 'team', label: 'Team · €8 a person', description: 'Shared notebooks, admin controls' },
];

/** Each option a card: the whole card chooses it. */
export function Plans() {
  const plan = useLocal<string | null>('plan', 'plus');
  return <RadioGroup bind={plan} label="Plan" items={plans} variant="card" data-testid="plan" />;
}
```

### Short options with help text

```tsx
import { useLocal } from '@lingxia/view';
import { RadioGroup } from '@liulidesign/core/RadioGroup';

/** Short options, no second line. */
export function Plain() {
  const theme = useLocal<string | null>('theme', 'system');
  return (
    <RadioGroup
      bind={theme}
      label="Appearance"
      description="System follows the device."
      items={[{ value: 'system', label: 'System' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }]}
      data-testid="appearance"
    />
  );
}
```

### Required, with an error

```tsx
import { useLocal } from '@lingxia/view';
import { RadioGroup } from '@liulidesign/core/RadioGroup';

/** Nothing chosen yet, required, after a failed send. */
export function Required() {
  const reason = useLocal<string | null>('reason', null);
  return (
    <RadioGroup
      bind={reason}
      label="Why are you leaving?"
      isRequired
      status={reason.value === null ? { type: 'error', message: 'Choose a reason.' } : undefined}
      items={[{ value: 'price', label: 'It costs too much' }, { value: 'features', label: 'It is missing something I need' }, { value: 'other', label: 'Something else' }]}
      data-testid="reason"
    />
  );
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { RadioGroup } from '@liulidesign/core/RadioGroup';

export function Sizes() {
  const pick = useLocal<string | null>('pick', 'a');
  const options = [{ value: 'a', label: 'Chosen' }, { value: 'b', label: 'Not chosen' }];
  return (
    <view className="gap-5">
      <RadioGroup bind={pick} label="Small" size="sm" items={options} />
      <RadioGroup bind={pick} label="Medium" items={options} />
      <RadioGroup bind={pick} label="Large" size="lg" items={options} />
    </view>
  );
}
```

### Side by side

```tsx
import { useLocal } from '@lingxia/view';
import { RadioGroup } from '@liulidesign/core/RadioGroup';

/** Two short options side by side. */
export function Inline() {
  const billing = useLocal<string | null>('billing', 'yearly');
  return (
    <RadioGroup
      bind={billing}
      label="Billing"
      orientation="horizontal"
      items={[{ value: 'monthly', label: 'Monthly' }, { value: 'yearly', label: 'Yearly' }]}
      description="Yearly saves two months."
      data-testid="billing"
    />
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
| `info` | `FieldInfo` |  | An info button after the label that opens a note. For what most people need, use `description`: it is always in view. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `bind` | `Local<string \| null>` |  | A View-local cell the group reads and writes; use it or `value` with `onChange`. |
| `value` | `string \| null` |  | The chosen option's value; `null` while none is chosen. |
| `onChange` | `((value: string) => void)` |  | Runs with the option chosen by a tap or by arrowing onto it. |
| `items` (required) | `readonly RadioOption[]` |  | The choices, in order. |
| `variant` | `RadioGroupVariant` | `'list'` | `list` stacks plain rows; `card` makes each option a bordered card, for plans and other choices with a second line. |
| `orientation` | `RadioGroupOrientation` | `'vertical'` | `horizontal` sets two or three short options side by side; they wrap when the line is full. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group; each option gets `<testid>-<value>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| field | `<Field>` | yes | Field: the group label, help text and status. |
| group | `<view>` | yes | The radio group: one Tab stop, named by the label. |
| option | `<button>` | yes | One radio per option; the whole row or card is the press target. |
| circle | `<view>` | yes | Drawn from the choice: filled with the accent and a dot when chosen. |
| label | `<text>` | yes | The option's text and its name. |
| description | `<text>` | no | A second line, read as the option's description. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `radioGroupRecipe` from `@liulidesign/core/RadioGroup/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-solid`, `danger`, `focus`, `line-strong`, `on-accent-solid`, `surface`, `text`, `text-muted`.

## Accessibility

- Pattern: radioGroup.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: checked (from value or bind, on the chosen option); disabled (from isDisabled, or the option's isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- Tab: Into the group, on the chosen option or else the first..
- Up / Left, Down / Right: The previous or next option, chosen on arrival; wraps at the ends..
- Press target: sm 32 px, md 40 px, lg 48 px.
- A card shows the choice twice, in its edge and in the circle, so it never rests on colour alone.
- With nothing chosen, Tab lands on the first option without choosing it.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| circle boundary, not chosen | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| circle fill, chosen | `accent-solid` on `surface` | 3:1 | 5.62:1 | 3.50:1 |
| chosen card edge | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: At large text, expect the circle to keep its size, centred on the label's first line as that line grows; the label wraps, the mark is never squeezed.
- Do: Choose a sensible default when one exists; leave the group empty only when the person must decide.
- Do: Use cards when each option needs a second line: plans, delivery, shipping speed.
- Don't: Use radios for more than about six options; use a Select.
- Don't: Use radios for a choice that takes effect at once in a toolbar; use Tabs in the segmented variant.

Related: CheckboxInput, Field, Tabs.
