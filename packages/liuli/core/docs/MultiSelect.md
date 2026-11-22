<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# MultiSelect

Several choices from a list, shown as tokens. Status: preview.

```ts
import { MultiSelect } from '@liulidesign/core/MultiSelect';
```

MultiSelect shows what is chosen as Tokens inside a field box and opens the whole list from a button at its end, each option with a check. A token's own button removes it without opening the list. It carries Field's label, help text and status. For a handful of options that fit on the page, a CheckboxList keeps them all in view.

## Examples

### Labels for a note

```tsx
import { useLocal } from '@lingxia/view';
import { MultiSelect } from '@liulidesign/core/MultiSelect';

const labels = [
  { value: 'travel', label: 'Travel' },
  { value: 'work', label: 'Work' },
  { value: 'recipes', label: 'Recipes' },
  { value: 'garden', label: 'Garden' },
  { value: 'money', label: 'Money' },
  { value: 'archive', label: 'Archive', isDisabled: true },
];

/** Labels for a note: the chosen ones are tokens in the field, the list opens from its end. */
export function Labels() {
  const chosen = useLocal<readonly string[]>('chosen', ['travel', 'recipes']);
  return (
    <MultiSelect
      bind={chosen}
      label="Labels"
      items={labels}
      addLabel="Add a label"
      removeLabel={name => `Remove ${name}`}
      description="A note can carry several."
      data-testid="labels"
    />
  );
}
```

### Nothing chosen, with an error

```tsx
import { useLocal } from '@lingxia/view';
import { MultiSelect } from '@liulidesign/core/MultiSelect';

/** Nothing chosen yet, on a required field after a failed send. */
export function Empty() {
  const chosen = useLocal<readonly string[]>('none', []);
  return (
    <MultiSelect
      bind={chosen}
      label="Share with teams"
      items={[{ value: 'design', label: 'Design' }, { value: 'eng', label: 'Engineering' }, { value: 'support', label: 'Support' }]}
      placeholder="No teams yet"
      addLabel="Add a team"
      removeLabel={name => `Remove ${name}`}
      isRequired
      status={chosen.value.length === 0 ? { type: 'error', message: 'Choose at least one team.' } : undefined}
      data-testid="teams"
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
| `bind` | `Local<readonly string[]>` |  | A View-local cell holding the chosen values; use it or `value` with `onChange`. |
| `value` | `readonly string[]` |  | The chosen options' values. |
| `onChange` | `((value: readonly string[]) => void)` |  | Runs with the chosen values, in the options' order. |
| `items` (required) | `readonly MultiSelectOption[]` |  | The choices, in order. |
| `placeholder` | `string` |  | Shown in the box while nothing is chosen. |
| `addLabel` | `string` |  | The name of the button that opens the list: "Add a label". Defaults to "Add" in the page's language. |
| `removeLabel` | `((label: string) => string)` |  | Each token's remove button, named with what it removes: `label => \`Remove ${label}\``. Defaults to "Remove <label>" in the page's language. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the box; the add button gets `<testid>-add`, each option `<testid>-<value>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | The field box: a group named by the label, holding the tokens. |
| placeholder | `<text>` | no | Shown while nothing is chosen. |
| add | `<DropdownMenu>` | yes | A DropdownMenu at the end of the box, named by `addLabel`, with a checkbox item per option. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `multiSelectRecipe` from `@liulidesign/core/MultiSelect/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-body`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `danger`, `line-strong`, `surface`, `text`, `text-muted`, `warning`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, then status.
- States: disabled (from isDisabled, or isDisabled on an option).
- Tab: To each token's remove button, then to the add button.
- Enter or Space on the add button: Opens the list.
- Enter or Space on an option: Checks or unchecks it; the list stays open.
- Escape: Closes the list.
- Press target: sm 24 px.
- The box is a group named by the field's label and described by its help and status.
- Each option is a menu item that reports checked; each token's remove button says what it removes.
- The chosen values keep the options' order, so the tokens do not jump as more are added.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| boundary | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it when the list is too long to leave open, or the field must stay one line tall when little is chosen.
- Do: Name the add button for what is added: "Add a label".
- Don't: Use it for two to six options that fit; a CheckboxList shows them all.
- Don't: Use it for one choice; that is a Select or a Combobox.

## Limits

- The list is not searched by typing, and new values cannot be typed in: it chooses from the options given.
- The error state colours the boundary but is not reported as invalid on the group.

Related: Token, CheckboxList, Combobox, Select.
