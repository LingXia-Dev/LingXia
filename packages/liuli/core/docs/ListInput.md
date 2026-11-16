<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ListInput

A field of rows the user adds and removes. Status: preview.

```ts
import { ListInput } from '@liulidesign/core/ListInput';
```

ListInput is one field whose value is a list: phone numbers, links, line items. Each row holds the app's own controls with a remove button at its end, and a button under the rows adds one. The app owns the rows and draws each one; ListInput gives them a shared label, help text and status, reads them as a list, and keeps the count between `min` and `max`.

## Examples

### Rows of two controls, between one and four

```tsx
import { useLocal } from '@lingxia/view';
import { Select } from '@liulidesign/core/Select';
import { TextInput } from '@liulidesign/core/TextInput';
import { ListInput } from '@liulidesign/core/ListInput';

const kinds = [{ value: 'mobile', label: 'Mobile' }, { value: 'home', label: 'Home' }, { value: 'work', label: 'Work' }];

/** Phone numbers: each row is a kind and a number; one row always stays, and four is the most. */
export function Phones() {
  const phones = useLocal<readonly Phone[]>('phones', [
    { id: 'p1', kind: 'mobile', number: '912 345 678' },
    { id: 'p2', kind: 'work', number: '' },
  ]);
  const next = useLocal('next', 3);
  const change = (index: number, patch: Partial<Phone>) => phones.set(phones.value.map((phone, at) => (at === index ? { ...phone, ...patch } : phone)));
  return (
    <ListInput
      label="Phone numbers"
      description="We call only about a delivery."
      rowKeys={phones.value.map(phone => phone.id)}
      renderRow={index => (
        <view className="flex-row gap-2">
          <Select label={`Kind of phone ${index + 1}`} isLabelHidden options={kinds} value={phones.value[index].kind} onChange={kind => change(index, { kind })} className="w-28" />
          <TextInput label={`Phone ${index + 1}`} isLabelHidden type="tel" placeholder="Number" value={phones.value[index].number} onChange={number => change(index, { number })} className="grow basis-0" data-testid={`phone-${index + 1}`} />
        </view>
      )}
      onAdd={() => { phones.set([...phones.value, { id: `p${next.value}`, kind: 'mobile', number: '' }]); next.set(next.value + 1); }}
      addLabel="Add a phone number"
      onRemove={index => phones.set(phones.value.filter((_, at) => at !== index))}
      removeLabel={index => `Remove phone ${index + 1}`}
      min={1}
      max={4}
      data-testid="phones"
    />
  );
}
```

### Empty and required

```tsx
import { useLocal } from '@lingxia/view';
import { TextInput } from '@liulidesign/core/TextInput';
import { ListInput } from '@liulidesign/core/ListInput';

/** Nothing yet, and required: the error says what to add. */
export function Links() {
  const links = useLocal<readonly { id: string; url: string }[]>('links', []);
  const next = useLocal('next', 1);
  return (
    <ListInput
      label="Links"
      isRequired
      status={links.value.length === 0 ? { type: 'error', message: 'Add at least one link.' } : undefined}
      rowKeys={links.value.map(link => link.id)}
      renderRow={index => (
        <TextInput label={`Link ${index + 1}`} isLabelHidden type="text" autoCapitalize="none" placeholder="https://" value={links.value[index].url} onChange={url => links.set(links.value.map((link, at) => (at === index ? { ...link, url } : link)))} />
      )}
      onAdd={() => { links.set([...links.value, { id: `l${next.value}`, url: '' }]); next.set(next.value + 1); }}
      addLabel="Add a link"
      onRemove={index => links.set(links.value.filter((_, at) => at !== index))}
      removeLabel={index => `Remove link ${index + 1}`}
      data-testid="links"
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
| `rowKeys` (required) | `readonly string[]` |  | One stable key per row, in order: an id, never the row's place. The app owns the rows. |
| `renderRow` (required) | `(index: number) => Child` |  | A row's controls, by its place from 0. Give each control its own label, hidden: "Phone 2". |
| `onAdd` (required) | `() => void` |  | Adds a row at the end. |
| `addLabel` (required) | `string` |  | The words on the add button: "Add a phone number". |
| `onRemove` (required) | `(index: number) => void` |  | Takes the row at this place away. |
| `removeLabel` (required) | `(index: number) => string` |  | Each remove button, named with what it removes: `index => \`Remove phone ${index + 1}\``. |
| `max` | `number` |  | The add button is disabled once there are this many rows. |
| `min` | `number` | `0` | Rows cannot be removed at or below this many. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the list; the add button gets `<testid>-add`, each remove button `<testid>-remove-<key>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| list | `<view>` | no | The rows: a list named by the label; absent while there are none. |
| row | `<view>` | no | One row: the app's controls, then its remove button. |
| controls | `<view>` | no | What `renderRow` returns; it takes the row's width. |
| add | `<view>` | yes | The button that adds a row, under the rows. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `listInputRecipe` from `@liulidesign/core/ListInput/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, then status.
- States: disabled (from isDisabled; the add button at `max`, the remove buttons at `min`).
- Tab: Through each row's controls and its remove button, then to the add button.
- Enter or Space: Removes the row, or adds one.
- Press target: md 40 px.
- The rows are a list named by the label and described by the help text and status.
- Each control in a row needs its own name with the row's number, hidden: "Phone 2". The field's label names the list, not the controls.
- Each remove button says which row it removes.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| remove icon | `text` on `surface` | 3:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Start with one empty row when at least one is needed, and set `min` to 1.
- Do: Key rows by an id the app makes when a row is added, so a row keeps its text when another is removed.
- Don't: Use it for choosing among known options; that is MultiSelect or CheckboxList.
- Don't: Put more than two or three controls in a row on a phone; open the row in a sheet when it needs more.

## Limits

- Rows are not reordered by dragging.
- It does not move focus to a new row; the app's controls can ask for it.

Related: Field, MultiSelect, FormLayout.
