<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Stepper

Where someone is in a task with a known number of steps. Status: preview.

```ts
import { Stepper } from '@liulidesign/core/Stepper';
```

Stepper shows the steps of a task that must be done in order: a checkout, a sign-up, an import. Finished steps carry a tick, the step in progress is ringed, and what is ahead is quiet. With `onChange` a finished step is a button that goes back to it. The app holds the step and shows its content.

## Examples

### A task in four steps

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Stepper } from '@liulidesign/core/Stepper';

const states = { done: 'Done', current: 'Current step', upcoming: 'Not started' };
const checkout = [
  { key: 'cart', label: 'Cart', description: 'Three items' },
  { key: 'address', label: 'Address', description: 'Where to deliver' },
  { key: 'payment', label: 'Payment', description: 'Card or transfer' },
  { key: 'review', label: 'Review', description: 'Check and place the order' },
];

/** A task in four steps: finished steps go back, the button moves on. */
export function Checkout() {
  const at = useLocal('at', 1);
  return (
    <view className="gap-4">
      <Stepper label="Checkout steps" steps={checkout} value={at.value} stateLabels={states} onChange={at.set} data-testid="checkout" />
      <Button label={at.value >= checkout.length - 1 ? 'Place order' : 'Continue'} variant="primary" onClick={() => at.set(Math.min(checkout.length, at.value + 1))} data-testid="continue" />
    </view>
  );
}
```

### Across the top of a form

```tsx
import { Stepper } from '@liulidesign/core/Stepper';

const states = { done: 'Done', current: 'Current step', upcoming: 'Not started' };

/** Across the top of a wide form: up to four short labels. */
export function Across() {
  return <Stepper label="Sign-up steps" orientation="horizontal" steps={[{ key: 'account', label: 'Account' }, { key: 'profile', label: 'Profile' }, { key: 'done', label: 'Done' }]} value={1} stateLabels={states} data-testid="across" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the steps for assistive tech: "Checkout steps". |
| `steps` (required) | `readonly StepperStep[]` |  | The steps, in order. |
| `value` (required) | `number` |  | The index of the step in progress, from 0; `steps.length` when every step is done. |
| `stateLabels` (required) | `Record<StepState, string>` |  | What a screen reader hears for a step's state: `{ done: 'Done', current: 'Current step', upcoming: 'Not started' }`. |
| `onChange` | `((index: number) => void)` |  | Makes a finished step a button that goes back to it; runs with its index. |
| `orientation` | `StepperOrientation` | `'vertical'` | `horizontal` suits up to four short labels; `vertical` any number, with descriptions. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## StepperStep props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `key` (required) | `string` |  | Identifies the step: its test id is `<id>-<key>`. |
| `label` (required) | `string` |  |  |
| `description` | `string` |  | A second line; vertical only. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The steps, a list named by `label`. |
| step | `<view>` | yes | One step; a `button` when it is finished and `onChange` is set. |
| marker | `<view>` | yes | A 24 px circle: a tick when done, the step's number otherwise. |
| line | `<view>` | no | Joins a step to the next; filled once the step is done. |
| label | `<text>` | yes | The step's name; heavier on the step in progress. |
| description | `<text>` | no | A second line under the label; vertical only. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `stepperRecipe` from `@liulidesign/core/Stepper/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `focus`, `line-strong`, `on-accent`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Through the finished steps, when they can be pressed.
- Enter or Space: Goes back to the step.
- Press target: .
- The steps are a list, so a screen reader says how many there are and where each stands.
- Each step is read as its name and its state in the app's words: "Address, Current step". The markers and lines are not read.
- State is carried by the marker's shape (tick, ring, thin ring) and the label's weight, not by colour alone.
- A vertical step that can be pressed is as tall as its two lines, about 44 px; a horizontal one is smaller, so offer a Back button as well.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| tick on a finished step | `on-accent` on `accent` | 3:1 | 5.62:1 | 7.12:1 |
| number and ring of the step in progress | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| ring of a step ahead | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| label of a step ahead, numbers and second lines | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it when the steps are known up front and done in order.
- Do: Use `vertical` on a phone and wherever steps have descriptions; `horizontal` for up to four short labels on a wide form.
- Do: Name steps by what is done in them: "Address", not "Step 2".
- Do: Set `current` to the number of steps when the task is finished, so every step is ticked.
- Don't: Use it to move between pages that can be visited in any order; that is Tabs.
- Don't: Let a step ahead be pressed; validate the current step, then move on.
- Don't: Use it for a single progress value; that is a ProgressBar.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A step marked as failed or optional. | nothing from the View | Done, in progress or ahead. |
| Horizontal steps that fold to "2 of 4" where they do not fit. | nothing from the View | Labels are cut short; use `vertical` on a phone. |

Related: ProgressBar, Tabs, Breadcrumb.
