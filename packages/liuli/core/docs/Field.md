<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Field

Label, help text and validation status around one form control. Status: preview.

```ts
import { Field } from '@liulidesign/core/Field';
```

Field lays out the parts every form control shares: a label, the control, help text and a status message. Liuli form controls use it inside; wrap a raw primitive in it only when Liuli has no component for that control yet.

## Examples

### Wrapping a primitive select

```tsx
import { useLocal } from '@lingxia/view';
import { Field } from '@liulidesign/core/Field';

const periods = [
  { value: 'monthly', label: 'Monthly' },
  { value: 'yearly', label: 'Yearly' },
] as const;

/** A raw select takes Field's label, help and status until Liuli has its own. */
export function WrappedSelect() {
  const period = useLocal('period', '');
  return (
    <Field
      label="Billing period"
      description="You can change it later."
      status={period.value ? undefined : { type: 'warning', message: 'Choose a period to see the price.' }}
      isRequired
      data-testid="field"
    >
      {control => (
        <select
          bind={period}
          aria-label={control['aria-label']}
          aria-describedby={control['aria-describedby']}
          aria-invalid={control['aria-invalid']}
          aria-required={control['aria-required']}
          disabled={control.disabled}
          placeholder="Choose…"
          options={periods}
          data-testid="period"
          className="min-h-control-md px-3 rounded-control border border-line-strong bg-surface text-body text-text"
        />
      )}
    </Field>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `(control: FieldControl) => Child` |  | Renders the control from the props Field gives it; pass each one on to the control element. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |
| `label` (required) | `string` |  | The visible label and the control's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Hides the visible label; the control keeps it as its name. |
| `description` | `string` |  | Help text under the control, read as the control's description. |
| `status` | `FieldStatus` |  | An `error` status marks the control invalid; its message is read as the control's description. |
| `isRequired` | `boolean` | `false` | Marks the label with an asterisk and the control as required. Validation is the app's. |
| `hasRequiredMark` | `boolean` | `true` | Draws the asterisk on a required label; turn it off on a form where every field is required. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| label | `<text>` | yes | The visible label; hidden from assistive tech because the control carries the same text as its name. |
| required | `<span>` | no | An asterisk after the label when `isRequired`. |
| description | `<text>` | no | Help text, or `disabledMessage` while disabled. |
| status | `<view>` | yes | A polite live region, always mounted; it shows an icon and the message when `status` has one. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: disabled (from isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- Field hands its control the name, the description ids and the states; pass each one on to the control element.
- The status region also announces a message politely when it appears or changes.

| Contrast | Pair | Minimum | Proven by the theme pack |
| --- | --- | --- | --- |
| label | `text` on `canvas` | 4.5:1 | yes |
| help text | `text-muted` on `canvas` | 4.5:1 | yes |
| status icon | `danger` on `canvas` | 3:1 | no |

## Guidance

- Do: Pass every prop Field hands the control on to the control element.
- Do: Say what to fix in a status message, not only that something is wrong.
- Don't: Use the placeholder as the label; it disappears as soon as someone types.
- Don't: Wrap a Liuli form control in a Field; it already has one.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Validation when the user leaves a field. | focus and blur events on editors | Validate on change or on submit. |

Related: TextInput, CheckboxInput.
