<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Tokenizer

Typed entries that become tokens. Status: preview.

```ts
import { Tokenizer } from '@liulidesign/core/Tokenizer';
```

Tokenizer is a field whose value is a list of words the user types: addresses, tags, keywords. Enter, a comma, a semicolon or a line break turns the text into a token, a pasted list splits into one token each, and leaving the field keeps what was typed. An entry already there is not added twice. With `validate`, a refused entry stays in the editor with the reason as the field's error. It carries Field's label, help text and status.

## Examples

### Email addresses, checked as they are entered

```tsx
import { useLocal } from '@lingxia/view';
import { Tokenizer } from '@liulidesign/core/Tokenizer';

const isAddress = (text: string) => (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text) ? undefined : `"${text}" is not an email address.`);

/** Who an invitation goes to: type or paste addresses, separated by commas; one that is not an address stays to be fixed. */
export function Recipients() {
  const to = useLocal<readonly string[]>('to', ['mira@harbour.co']);
  return (
    <Tokenizer
      bind={to}
      label="Invite"
      placeholder="name@example.com"
      description="Press Enter or type a comma after each address."
      validate={isAddress}
      data-testid="invite"
    />
  );
}
```

### Free tags, small

```tsx
import { useLocal } from '@lingxia/view';
import { Tokenizer } from '@liulidesign/core/Tokenizer';

/** Tags for a post: free words, a comma ends each one. */
export function Tags() {
  const tags = useLocal<readonly string[]>('tags', ['ferries', 'summer']);
  return <Tokenizer bind={tags} label="Tags" placeholder="Add a tag" size="sm" data-testid="tags" />;
}
```

### Nothing entered, with an error

```tsx
import { useLocal } from '@lingxia/view';
import { Tokenizer } from '@liulidesign/core/Tokenizer';

/** Empty and required, after a failed send. */
export function Empty() {
  const words = useLocal<readonly string[]>('words', []);
  return (
    <Tokenizer
      bind={words}
      label="Keywords"
      placeholder="Words a search should match"
      isRequired
      status={words.value.length === 0 ? { type: 'error', message: 'Add at least one keyword.' } : undefined}
      data-testid="keywords"
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
| `bind` | `Local<readonly string[]>` |  | A View-local cell holding the entries; use it or `value` with `onChange`. |
| `value` | `readonly string[]` |  | The entries, in the order they were typed. |
| `onChange` | `((value: readonly string[]) => void)` |  | Runs with the entries after one is added or removed. |
| `placeholder` | `string` |  | Shown in the editor while nothing is typed and nothing is entered. |
| `validate` | `((text: string) => string \| undefined)` |  | Checks an entry before it becomes a token: return the reason it is refused, shown as the field's error, and the text stays in the editor to be corrected: `text => text.includes('@') ? undefined : 'Not an email address'`. |
| `removeLabel` | `((entry: string) => string)` |  | Each token's remove button, named with what it removes. Defaults to "Remove <entry>" in the page's language. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the editor; the box gets `<testid>-box`, each token `<testid>-token-<index>`. |

- `bind` | `value` + `onChange`: props from one side only.

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | The field box: a group named by the label, holding the tokens and the editor. |
| editor | `<input>` | yes | The text after the last token, named by the label; it wraps to a line of its own when the tokens fill the line. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `tokenizerRecipe` from `@liulidesign/core/Tokenizer/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `line-strong`, `surface`, `text`, `text-faint`, `text-muted`, `warning`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, then status.
- States: disabled (from isDisabled); invalid (from status, or an entry validate refused).
- Enter: Turns the typed text into a token.
- , or ;: Ends the entry before it, which becomes a token.
- Tab: To each token's remove button, then the editor; leaving the editor keeps what was typed.
- Press target: sm 24 px.
- The editor is named by the field's label and described by its help and status; a refused entry is announced as the field's error.
- Each token's remove button says what it removes.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| boundary | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it when the entries are the user's own words: addresses, tags, search terms.
- Do: Check addresses or codes with `validate`, so a typo is fixed before it becomes a token.
- Don't: Use it to choose from a known list; a MultiSelect offers the options.
- Don't: Use it for one value; that is a TextInput.

## Limits

- Backspace in an empty editor does not remove the last token; its own button does.
- It suggests nothing while typing; a Combobox suggests from a list.

Related: MultiSelect, Token, TextInput, Combobox.
