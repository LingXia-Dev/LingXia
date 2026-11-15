<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# CheckboxList

Several on/off choices under one question, any number of which can be chosen. Status: preview.

```ts
import { CheckboxList } from '@liulidesign/core/CheckboxList';
```

CheckboxList asks one question and offers a short list of answers that can each be on or off: which notifications, which days, which toppings. Its value is the list of chosen values, in the options' order. The question names the group for assistive tech, and help text and a status message speak for the group as a whole, such as "Choose at least one".

## Examples

### Any number of options

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxList } from '@liulidesign/core/CheckboxList';

const topics = [
  { value: 'mentions', label: 'Mentions', description: 'When someone names you in a note.' },
  { value: 'comments', label: 'Comments', description: 'Replies on notes you wrote.' },
  { value: 'digest', label: 'Weekly digest' },
];

/** Any number of the options can be on; the value is the list of those that are. */
export function Notifications() {
  const chosen = useLocal<readonly string[]>('topics', ['mentions']);
  return (
    <view className="gap-3">
      <CheckboxList label="Notify me about" bind={chosen} options={topics} data-testid="topics" />
      <text role="status" data-testid="chosen" className="text-footnote text-muted">{chosen.value.length === 0 ? 'Nothing chosen' : chosen.value.join(', ')}</text>
    </view>
  );
}
```

### A rule about the group

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxList } from '@liulidesign/core/CheckboxList';

const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri'].map(day => ({ value: day.toLowerCase(), label: day }));

/** A message about the group as a whole, when the choice so far cannot stand. */
export function AtLeastOne() {
  const chosen = useLocal<readonly string[]>('days', []);
  return (
    <CheckboxList
      label="Office days"
      description="We book a desk for each day you choose."
      isRequired
      bind={chosen}
      options={days}
      status={chosen.value.length === 0 ? { type: 'error', message: 'Choose at least one day.' } : undefined}
      data-testid="days"
    />
  );
}
```

### An option that cannot be chosen

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxList } from '@liulidesign/core/CheckboxList';

const toppings = [
  { value: 'olives', label: 'Olives' },
  { value: 'mushrooms', label: 'Mushrooms' },
  { value: 'anchovies', label: 'Anchovies', description: 'Sold out today.', isDisabled: true },
];

export function WithUnavailable() {
  const chosen = useLocal<readonly string[]>('toppings', ['olives']);
  return <CheckboxList label="Toppings" bind={chosen} options={toppings} data-testid="toppings" />;
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { CheckboxList } from '@liulidesign/core/CheckboxList';

export function Sizes() {
  const chosen = useLocal<readonly string[]>('sized', ['a']);
  const two = [{ value: 'a', label: 'First' }, { value: 'b', label: 'Second' }];
  return (
    <view className="grid grid-cols-2 gap-4">
      <CheckboxList label="Small" size="sm" bind={chosen} options={two} data-testid="small" />
      <CheckboxList label="Large" size="lg" bind={chosen} options={two} data-testid="large" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<readonly string[]>` |  | A View-local cell holding the chosen values; use it or `value` with `onChange`. |
| `value` | `readonly string[]` |  | The chosen options' values. |
| `onChange` | `((value: readonly string[]) => void)` |  | Runs with the chosen values, in the options' order. |
| `label` (required) | `string` |  | The question the options answer, and the group's accessible name: "Notify me about". |
| `isLabelHidden` | `boolean` | `false` | Hides the visible question; the group keeps it as its name. |
| `description` | `string` |  | Help text under the question, read as the group's description. |
| `status` | `FieldStatus` |  | A message about the group as a whole: "Choose at least one". |
| `isRequired` | `boolean` | `false` | Marks the question with an asterisk. Validation is the app's. |
| `options` (required) | `readonly CheckboxListOption[]` |  |  |
| `isDisabled` | `boolean` | `false` | Disables every option. |
| `size` | `FieldSize` | `'md'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group; an option's checkbox is `<id>-<value>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The group, named by `label`. |
| label | `<text>` | yes | The question; hidden from assistive tech, since the group carries it as its name. |
| description | `<text>` | no | Help text under the question, read as the group's description. |
| options | `<view>` | yes | The checkboxes, one CheckboxInput each. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, then status message.
- States: checked (from value, on each option); disabled (from isDisabled, or an option's own); invalid (from status.type === "error", on the group's message).
- Tab: Moves from one checkbox to the next; each is its own stop.
- Space: Turns the focused checkbox on or off.
- A group named by `label`; each option is a checkbox named by its own label and described by its own help text.
- The group's status message is a live region under the options: it is announced when it appears or changes.
- Checkboxes are separate Tab stops, unlike radio buttons: each is its own answer.
- An option that cannot be chosen stays in the list, dimmed, with its reason as help text.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| question and option labels | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| help text | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Word `label` as the question the options answer: "Notify me about".
- Do: Keep it to about seven options; past that, group them or use a searchable list.
- Do: Put a rule about the whole group ("Choose at least one") in `status`, shown once the choice breaks it.
- Do: Keep an unavailable option visible with `isDisabled` and say why in its `description`.
- Don't: Use it when only one answer is allowed; that is a RadioGroup.
- Don't: Use it for a single on/off setting; that is a CheckboxInput or a Switch.
- Don't: Reorder options when they are chosen.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A "select all" checkbox that shows a dash while some options are on. | a mixed checked state on a checkbox | An app adds its own button to choose all or none. |
| Options laid out in a row or in columns. | nothing from the View | One column. |

Related: CheckboxInput, RadioGroup, Switch.
