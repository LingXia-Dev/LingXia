<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# InputGroup

A field joined edge to edge with fixed text, a select or a button, as one control. Status: preview.

```ts
import { InputGroup, InputGroupText } from '@liulidesign/core/InputGroup';
```

InputGroup joins a TextInput with what completes it: fixed text such as a scheme or a unit (InputGroupText), a Select such as a currency, or the Button that acts on it. Neighbours share an edge and only the group's two ends are round; the text field takes the room the others leave. Each control keeps its own name, and the group is named once.

## Examples

### Fixed text before the field

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';
import { InputGroup, InputGroupText } from '@liulidesign/core/InputGroup';

/** Fixed text before the field: the scheme is part of the address, never typed. */
export function Website() {
  const site = useLocal('site', '');
  return (
    <InputGroup label="Website" data-testid="website">
      <InputGroupText>https://</InputGroupText>
      <TextInput bind={site} label="Website" placeholder="example.com" type="text" inputMode="text" autoCapitalize="none" data-testid="site" />
    </InputGroup>
  );
}
```

### A currency and an amount

```tsx
import { useLocal } from '@lingxia/view';
import { Select } from '@liulidesign/core/Select';
import { TextInput } from '@liulidesign/core/TextInput';
import { InputGroup } from '@liulidesign/core/InputGroup';

const currencies = [{ value: 'usd', label: 'USD' }, { value: 'eur', label: 'EUR' }, { value: 'jpy', label: 'JPY' }];

/** A currency and an amount entered as one price. */
export function Price() {
  const currency = useLocal('currency', 'usd');
  const amount = useLocal('amount', '');
  return (
    <InputGroup label="Price" data-testid="price">
      <Select bind={currency} label="Currency" items={currencies} data-testid="currency" />
      <TextInput bind={amount} label="Amount" placeholder="0.00" inputMode="decimal" data-testid="amount" />
    </InputGroup>
  );
}
```

### A field and its action

```tsx
import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { TextInput } from '@liulidesign/core/TextInput';
import { InputGroup } from '@liulidesign/core/InputGroup';

/** A field and the action it feeds: the button ends the group. */
export function Invite() {
  const email = useLocal('email', '');
  return (
    <Said>
      {say => (
        <InputGroup label="Invite by email" data-testid="invite">
          <TextInput bind={email} label="Email" placeholder="name@example.com" type="email" autoCapitalize="none" data-testid="email" />
          <Button label="Invite" onClick={() => say(email.value ? `Invited ${email.value}` : 'Add an email first')} data-testid="send" />
        </InputGroup>
      )}
    </Said>
  );
}
```

### A unit after the value, at two sizes

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';
import { InputGroup, InputGroupText } from '@liulidesign/core/InputGroup';

/** A unit after the value, and a size for the whole group. */
export function Sizes() {
  const [small, large] = [useLocal('small', '72.4'), useLocal('large', '72.4')];
  return (
    <view className="gap-4">
      <InputGroup label="Weight, small" size="sm" data-testid="small">
        <TextInput bind={small} label="Weight" inputMode="decimal" />
        <InputGroupText>kg</InputGroupText>
      </InputGroup>
      <InputGroup label="Weight" data-testid="medium">
        <TextInput bind={large} label="Weight" inputMode="decimal" />
        <InputGroupText>kg</InputGroupText>
      </InputGroup>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | What the joined controls enter together, read once as the group's name: "Website", "Price". Each control keeps its own name. |
| `children` (required) | `Child` |  | TextInput, Select, Button, IconButton and InputGroupText elements, in reading order. |
| `size` | `FieldSize` |  | The size of every control that sets none. By default `md`, or `sm` where the screen is compact. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row, a group assistive tech names by `label`. Each control rounds only the group's two ends. |
| text | `<view>` | no | InputGroupText: fixed text on a sunken box with the field's edge, so it reads as part of the value. |
| textLabel | `<text>` | no | The fixed text, muted and never cut short. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `inputGroupRecipe` from `@liulidesign/core/InputGroup/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `line-strong`, `surface-sunken`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Moves through the controls in the group in reading order.
- Role group, named by `label`; each control keeps its own name, so its visible label is left out and read instead.
- A focused field keeps its accent on the edge it shares: the second pixel of its focus edge is drawn inside it.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| the fixed text | `text-muted` on `surface-sunken` | 4.5:1 | 5.20:1 | 7.45:1 |
| the joined edges | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Join only what makes one value: a scheme and a host, a currency and an amount, a field and its one action.
- Do: Give every control a `label`: it is what a screen reader says for it inside the group.
- Do: Use InputGroupText for text the user never types; use TextInput's own `prefix` or `suffix` when the text belongs inside the field.
- Don't: Join two text fields; give each its own label above it.
- Don't: Put a primary button in the group; the joined button is secondary, so the group stays one quiet shape.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A status message under the whole group. | nothing from the View | A control in the group shows its error edge; say what is wrong in text near the group. |

Related: TextInput, Select, ButtonGroup.
