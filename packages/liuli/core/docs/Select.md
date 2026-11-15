<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Select

One choice from a list that opens on demand. Status: preview.

```ts
import { Select } from '@liulidesign/core/Select';
```

Select picks one option from a list too long to show in place: a country, a role, a sort order. It is a Field around the select primitive, which opens the View's own list: under the box with a mouse or trackpad, as a sheet with Cancel and Done on touch.

## Examples

### With a placeholder and help text

```tsx
import { useLocal } from '@lingxia/view';
import { Select } from '@liulidesign/core/Select';

const countries = [
  { value: 'de', label: 'Germany' },
  { value: 'fr', label: 'France' },
  { value: 'it', label: 'Italy' },
  { value: 'nl', label: 'Netherlands' },
  { value: 'es', label: 'Spain' },
];

/** A label, a placeholder until chosen, and help text. */
export function Country() {
  const country = useLocal('country', '');
  return (
    <Select
      bind={country}
      label="Country"
      placeholder="Choose a country"
      description="Sets the currency and the tax on your invoices."
      options={countries}
      data-testid="country"
    />
  );
}
```

### Required, with an error and a disabled option

```tsx
import { useLocal } from '@lingxia/view';
import { Select } from '@liulidesign/core/Select';

/** Required: an error after a send with nothing chosen, cleared by a choice. */
export function Required() {
  const role = useLocal('role', '');
  const tried = useLocal('tried', true);
  return (
    <Select
      bind={role}
      label="Role"
      isRequired
      placeholder="Choose a role"
      options={[{ value: 'viewer', label: 'Can view' }, { value: 'editor', label: 'Can edit' }, { value: 'owner', label: 'Owner', isDisabled: true }]}
      status={tried.value && role.value === '' ? { type: 'error', message: 'Choose a role for the invite.' } : undefined}
      description="Only an owner can add another owner."
      data-testid="role"
    />
  );
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { Select } from '@liulidesign/core/Select';

export function Sizes() {
  const zone = useLocal('zone', 'cet');
  const options = [{ value: 'cet', label: 'Central European Time' }, { value: 'utc', label: 'Coordinated Universal Time' }];
  return (
    <view className="gap-4">
      <Select bind={zone} label="Small" size="sm" options={options} />
      <Select bind={zone} label="Medium" options={options} />
      <Select bind={zone} label="Large" size="lg" options={options} />
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
| `info` | `FieldInfo` |  | An info button after the label that opens a note. For what most people need, use `description`: it is always in view. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `bind` | `Local<string>` |  | A View-local cell the select reads and writes; use it or `value` with `onChange`. |
| `value` | `string` |  | The chosen option's value; `''` shows the placeholder. |
| `onChange` | `((value: string) => void)` |  | Runs with the chosen option's value. |
| `options` (required) | `readonly SelectOption[]` |  | The choices, in order; the platform shows them as a list or a menu. |
| `placeholder` | `string` |  | Shown in the box while nothing is chosen. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the select. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| field | `<Field>` | yes | Field: the label, help text and status. |
| control | `<select>` | yes | The select primitive: the box, the chosen label or placeholder, the chevron, and the list it opens. |

## Accessibility

- Pattern: select.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: disabled (from isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- Enter, Space: Opens the list..
- Up, Down: Moves through the open list; Enter chooses, Escape closes..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The placeholder is never the name: the label is, and it stays when the label is hidden.
- The open list is a listbox of options: a popover under the box with a fine pointer, a modal sheet on touch, where a tap picks and Done commits.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| box boundary | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| chosen label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use a select for seven or more options, or when the options matter less than the space.
- Do: Write the placeholder as an instruction ("Choose a country"), not as an option.
- Don't: Use a select for two to six options that fit; use a RadioGroup so they can be compared.
- Don't: Use a select to run an action; use a DropdownMenu.
- Don't: Ask a select to search or to clear its value; that is a Combobox.

Related: Combobox, RadioGroup, DropdownMenu, Field.
