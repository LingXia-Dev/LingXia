<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# TextInput

Single-line text field with label, help text, status and an optional clear button. Status: preview.

```ts
import { TextInput } from '@liulidesign/core/TextInput';
```

TextInput is a labelled one-line editor for names, emails, codes, amounts and searches. Bind it to a `useLocal` cell for View-local drafts, or pass `value` and `onChange` when the app owns the text.

## Examples

### Bound to local state, with validation

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';

/** Bound to a View-local cell: no Logic round trip per keystroke. */
export function Bound() {
  const email = useLocal('email', '');
  const invalid = email.value !== '' && !email.value.includes('@');
  return (
    <TextInput
      bind={email}
      label="Email"
      placeholder="you@example.com"
      type="email"
      autoComplete="email"
      autoCapitalize="none"
      enterKeyHint="next"
      description="We send the receipt here."
      status={invalid ? { type: 'error', message: 'Add the part after @, such as example.com.' } : undefined}
      isRequired
      data-testid="email"
    />
  );
}
```

### Search with a clear button

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';

/** Controlled (`value` + `onChange`), as a search field with a clear button the app names. */
export function Search() {
  const query = useLocal('query', '');
  return (
    <TextInput
      value={query.value}
      onChange={next => query.set(next)}
      label="Search notes"
      isLabelHidden
      placeholder="Search notes"
      type="search"
      hasClear
      clearLabel="Clear search"
      data-testid="search"
    />
  );
}
```

### Fixed text before and after the value, and an optional field

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';

/** Fixed text beside the value says its unit or its start, so the person types only what changes. */
export function Adornments() {
  const price = useLocal('price', '24.00');
  const site = useLocal('site', '');
  const weight = useLocal('weight', '');
  return (
    <view className="gap-4">
      <TextInput label="Price" bind={price} prefix="$" suffix="USD" inputMode="decimal" data-testid="price" />
      <TextInput label="Website" bind={site} prefix="https://" placeholder="example.com" autoCapitalize="none" data-testid="site" />
      <TextInput label="Weight" bind={weight} suffix="kg" inputMode="decimal" optionalLabel="optional" data-testid="weight" />
    </view>
  );
}
```

### Disabled, with the reason

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';

export function Disabled() {
  const code = useLocal('code', 'LX-2041');
  return <TextInput bind={code} label="Invite code" isDisabled disabledMessage="Codes are fixed once sent." data-testid="code" />;
}
```

### Checking what was typed

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';

/** The app is checking what was typed; the field stays editable. */
export function Checking() {
  const handle = useLocal('handle', 'ana');
  return <TextInput bind={handle} label="Username" prefix="@" description="Checking it is free." isLoading autoCapitalize="none" data-testid="handle" />;
}
```

### Read-only

```tsx
import { TextInput } from '@liulidesign/core/TextInput';

/** A value to read and copy, not to change. */
export function ReadOnly() {
  return <TextInput value="C-204188" onChange={() => {}} label="Customer ID" description="Set when the account was opened." isReadOnly data-testid="iban" />;
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
| `bind` | `Local<string>` |  | A View-local cell the control reads and writes; use it or `value` with `onChange`. |
| `value` | `string` |  | The app's value; pass `onChange` with it. |
| `onChange` | `((value: string) => void)` |  | Runs with the new value; the app passes it back as `value`. |
| `hasClear` | `boolean` | `false` | A button that empties the text, shown while there is text. |
| `clearLabel` | `string` |  | The clear button's accessible name; required with `hasClear`. |
| `placeholder` | `string` |  | A hint shown while empty; never a substitute for `label`. |
| `type` | `"email" \| "number" \| "password" \| "search" \| "tel" \| "text"` | `'text'` | `search` draws Liuli's magnifier, not the platform's, and defaults `enterKeyHint` to `search`. |
| `inputMode` | `"decimal" \| "email" \| "numeric" \| "tel" \| "text"` |  |  |
| `autoComplete` | `AutoComplete` |  |  |
| `enterKeyHint` | `"done" \| "enter" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  | The label of the keyboard's action key; `onSubmit` runs on it. |
| `autoCapitalize` | `"characters" \| "none" \| "sentences" \| "words"` |  |  |
| `hasSpellCheck` | `boolean` |  | Unset follows the platform. |
| `hasAutoFocus` | `boolean` | `false` | Focus it when it first appears. |
| `maxLength` | `number` |  |  |
| `isReadOnly` | `boolean` | `false` | Shows the value without letting it be changed; it can still be selected, copied and focused. For a value that does not apply, use `isDisabled`. |
| `isLoading` | `boolean` | `false` | A spinner at the end of the box while the app checks or fetches the value; the field reports busy and stays editable. |
| `startIcon` | `Child` |  | An icon inside the box, before the text; drawn 16 px in the muted colour. Replaces the search magnifier. |
| `prefix` | `string` |  | Fixed text inside the box before the value, as the app writes it: "$", "https://". Read with the description. |
| `suffix` | `string` |  | Fixed text inside the box after the value: a unit such as "kg" or "%". Read with the description. |
| `onSubmit` | `((value: string) => void)` |  | The keyboard's action key, or Enter. |
| `className` | `string` |  | Classes for the field column. |
| `controlClassName` | `string` |  | Classes for the bordered box. |
| `data-testid` | `string` |  | On the editor. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | The bordered box; it takes the focus colour while the editor has focus and the status colour otherwise. |
| icon | `<view>` | no | `startIcon`, or the magnifier when `type` is `search`; 16 px, muted, hidden from assistive tech. |
| input | `<input>` | yes | The editor; it fills the box height so taps above and below the text land in it. |
| clear | `<button>` | no | Empties the text; shown while there is text and `hasClear` is on. |

## What an app may override

- `className`: Classes for the field column.
- `controlClassName`: Classes for the bordered box.
- The recipe, `textInputRecipe` from `@liulidesign/core/TextInput/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `focus`, `line-strong`, `surface`, `surface-sunken`, `text`, `text-faint`, `text-muted`, `warning`.

## Accessibility

- Pattern: textbox.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: disabled (from isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- The keyboard action key, Enter: runs onSubmit.
- Press target: sm 32 px, md 40 px, lg 48 px.
- The clear button is a separate control named by `clearLabel`.
- Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| box boundary | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| focus boundary | `focus` on `surface` | 3:1 | 5.62:1 | 6.19:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Bind to useLocal for drafts; send the value to Logic on submit, not on every keystroke.
- Do: Set type, inputMode, autoComplete and enterKeyHint so the right keyboard and autofill appear.
- Do: Explain the fix in the status message.
- Don't: Leave out label because a placeholder is shown; use isLabelHidden when the label must not be drawn.
- Don't: Use type="number" for codes, phone numbers or card numbers; use inputMode="numeric".

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Validation on leaving the field; dirty tracking. | focus and blur events on editors | Validate as the value changes, or on submit. |
| Read-only text that can still be selected and copied. | readOnly on editors | Show the value as selectable text instead. |
| Moving focus to the next field from code when the app owns the value. | a focus handle for controlled editors | Bound fields chain with enterKeyHint="next" or cell.focus(). |

Related: Field.
