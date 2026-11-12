<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# CheckboxInput

One on/off choice with a label, help text and status. Status: preview.

```ts
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';
```

CheckboxInput is a single yes/no choice: opt-ins, terms, settings that take effect on save. The box, label and help text form one control, so a tap anywhere on them toggles it.

## Examples

### With help text

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';

export function Basic() {
  const digest = useLocal('digest', true);
  return <CheckboxInput bind={digest} label="Weekly digest" description="A summary of your notes every Monday." data-testid="digest" />;
}
```

### Owned by the app, with an error

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';

/** Owned by the app: the checkbox reports, the app decides. */
export function Controlled() {
  const accepted = useLocal('accepted', false);
  return (
    <CheckboxInput
      value={accepted.value}
      onChange={next => accepted.set(next)}
      label="I accept the terms of service"
      isRequired
      status={accepted.value ? undefined : { type: 'error', message: 'Accept the terms to create the account.' }}
      data-testid="terms"
    />
  );
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';

export function Sizes() {
  const small = useLocal('small', false);
  const medium = useLocal('medium', true);
  const large = useLocal('large', false);
  return (
    <view className="gap-2">
      <CheckboxInput bind={small} label="Small" size="sm" />
      <CheckboxInput bind={medium} label="Medium" />
      <CheckboxInput bind={large} label="Large" size="lg" />
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
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `bind` | `Local<boolean>` |  | A View-local cell the control reads and writes; use it or `value` with `onChange`. |
| `value` | `boolean` |  | The app's value; pass `onChange` with it. |
| `onChange` | `((value: boolean) => void)` |  | Runs with the new value; the app passes it back as `value`. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the checkbox. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| control | `<checkbox>` | yes | The checkbox primitive; the press target and the accessible node. |
| box | `<view>` | yes | The square Liuli draws from the value; the primitive draws none when it has content. |
| mark | `<vector>` | no | The check, shown while checked. |
| label | `<text>` | yes | The choice; visually hidden with `isLabelHidden`. |
| description | `<text>` | no | Help text, or `disabledMessage` while disabled. |

## Accessibility

- Pattern: checkbox.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: checked (from value or bind); disabled (from isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- The help text sits inside the control so a tap on it toggles, but it is read as the description, not the name.
- The status message sits outside the control in a polite live region and is also read as the description.
- The box is 16 to 24 px, but the press target is the whole row, label and help text included.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| box boundary, unchecked | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| box fill, checked | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| check mark | `on-accent` on `accent` | 3:1 | 5.62:1 | 7.12:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Phrase the label so that checked means yes.
- Do: Use the help text for the consequence, not for a second question.
- Don't: Act on the change at once when the screen has a Save button; apply on save.
- Don't: Put another control inside the label.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A mixed state for a select-all box. | a mixed checked state on the checkbox primitive | Two states only. |

Related: Field.
