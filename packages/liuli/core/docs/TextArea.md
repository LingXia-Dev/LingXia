<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# TextArea

Multi-line text field that grows with its text, with label, help text and status. Status: preview.

```ts
import { TextArea } from '@liulidesign/core/TextArea';
```

TextArea is a labelled editor for notes, comments, replies and descriptions. It shows `rows` lines, grows with the text up to `maxRows`, then scrolls. Bind it to a `useLocal` cell, or pass `value` and `onChange` when the app owns the text.

## Examples

### Bound to local state, growing to six lines

```tsx
import { useLocal } from '@lingxia/view';
import { TextArea } from '@liulidesign/core/TextArea';

/** Bound to a View-local cell; it grows with the text up to six lines, then scrolls. */
export function Bound() {
  const note = useLocal('note', '');
  return (
    <TextArea
      bind={note}
      label="Delivery note"
      placeholder="Gate code, where to leave it"
      description="The courier sees this when they arrive."
      rows={3}
      maxRows={6}
      maxLength={280}
      data-testid="note"
    />
  );
}
```

### A reply box that sends on Return

```tsx
import { useLocal } from '@lingxia/view';
import { TextArea } from '@liulidesign/core/TextArea';

/** A reply box: Send on the keyboard submits, Shift+Return adds a line. */
export function Reply() {
  const draft = useLocal('draft', '');
  const sent = useLocal('sent', '');
  return (
    <view className="gap-2">
      <TextArea
        value={draft.value}
        onChange={next => draft.set(next)}
        label="Reply"
        isLabelHidden
        placeholder="Write a reply"
        rows={1}
        maxRows={5}
        enterKeyHint="send"
        onSubmit={text => { sent.set(text); draft.set(''); }}
        data-testid="reply"
      />
      {sent.value !== '' && <text data-testid="sent" className="text-footnote text-muted">{sent.value}</text>}
    </view>
  );
}
```

### Required, with an error

```tsx
import { useLocal } from '@lingxia/view';
import { TextArea } from '@liulidesign/core/TextArea';

/** A required field with an error that names the fix. */
export function Invalid() {
  const reason = useLocal('reason', '');
  const short = reason.value.trim().length < 10;
  return (
    <TextArea
      bind={reason}
      label="Why are you leaving?"
      isRequired
      status={short ? { type: 'error', message: 'Write at least 10 characters.' } : undefined}
      data-testid="reason"
    />
  );
}
```

### With a character count

```tsx
import { useLocal } from '@lingxia/view';
import { TextArea } from '@liulidesign/core/TextArea';

/** A limit the writer can see coming. */
export function Counted() {
  const bio = useLocal('bio', 'Designer in Lisbon. I write about type, maps and small tools.');
  return <TextArea bind={bio} label="About you" description="Shown on your profile." maxLength={160} hasCount data-testid="bio" />;
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
| `placeholder` | `string` |  | A hint shown while empty; never a substitute for `label`. |
| `rows` | `number` | `3` | Lines shown while empty or short. |
| `maxRows` | `number` | `8` | Lines it grows to as the text gets longer; then it scrolls. Never below `rows`. |
| `inputMode` | `"decimal" \| "email" \| "numeric" \| "tel" \| "text"` |  |  |
| `autoComplete` | `AutoComplete` |  |  |
| `enterKeyHint` | `"done" \| "enter" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  | The keyboard's action key. `send`, `go`, `done` and `search` make Return submit and Shift+Return add a line. |
| `autoCapitalize` | `"characters" \| "none" \| "sentences" \| "words"` |  |  |
| `hasSpellCheck` | `boolean` |  | Unset follows the platform. |
| `hasAutoFocus` | `boolean` | `false` | Focus it when it first appears. |
| `maxLength` | `number` |  |  |
| `hasCount` | `boolean` | `false` | Shows how much of `maxLength` is used, at the bottom end of the box: "118 / 280". Needs `maxLength`. |
| `onSubmit` | `((value: string) => void)` |  | The keyboard's action key, or Return when `enterKeyHint` makes it submit. |
| `className` | `string` |  | Classes for the field column. |
| `controlClassName` | `string` |  | Classes for the bordered box. |
| `data-testid` | `string` |  | On the editor. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | The bordered box; it takes the focus colour while the editor has focus and the status colour otherwise. |
| editor | `<textarea>` | yes | The editor; it fills the box, shows `rows` lines and grows to `maxRows`. |

## What an app may override

- `className`: Classes for the field column.
- `controlClassName`: Classes for the bordered box.
- The recipe, `textAreaRecipe` from `@liulidesign/core/TextArea/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `line-strong`, `surface`, `text`, `text-faint`, `text-muted`, `warning`.

## Accessibility

- Pattern: textbox.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: disabled (from isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- Return: a new line; with enterKeyHint send, go, done or search it runs onSubmit.
- Shift+Return: a new line when Return submits.
- The editor is named by the label even when the label is hidden; the placeholder is not its name.
- Growing does not move focus or the caret; past `maxRows` the editor scrolls its own text.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| box boundary | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| focus boundary | `focus` on `surface` | 3:1 | 5.62:1 | 6.19:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it when the answer may run past one line: a note, a comment, a reason.
- Do: Set `rows` to the length you expect, so the box says how much to write.
- Do: Use enterKeyHint="send" for a chat or reply box, so Return sends and Shift+Return adds a line.
- Don't: Use it for a name, an email or a code; use TextInput.
- Don't: Let it grow without a `maxRows` in a sheet or a short screen; the keyboard covers the rest.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Read-only text that can still be selected and copied. | readOnly on editors | Show the value as selectable text instead. |
| Validation on leaving the field. | focus and blur events on editors | Validate as the value changes, or on submit. |

Related: TextInput, Field.
