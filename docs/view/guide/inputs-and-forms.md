# Inputs and forms

```tsx
import { useLocal, type View } from '@lingxia/ui';

type Actions = { signUp(form: { email: string; code: string; plan: string; news: boolean; seats: number }): Promise<void> };

export function render({ actions }: View<Actions>) {
  const email = useLocal('email', '');
  const code = useLocal('code', '');
  const plan = useLocal('plan', 'basic');
  const news = useLocal('news', true);
  const seats = useLocal('seats', 3);
  const busy = useLocal('busy', false);
  const field = 'px-3 py-2 rounded-lg border border-line bg-surface text-sm focus:border-accent';
  const submit = async () => {
    busy.set(true);
    try { await actions.signUp({ email: email.value, code: code.value, plan: plan.value, news: news.value, seats: seats.value }); }
    finally { busy.set(false); }
  };
  return (
    <page className="p-4 gap-3">
      <input bind={email} accessibilityLabel="Email" placeholder="you@example.com" keyboard="email"
        autoComplete="email" autoCapitalize="none" enterKeyHint="next" className={field} />
      <input bind={code} accessibilityLabel="Invite code" placeholder="6 digits" keyboard="number"
        maxLength={6} autoComplete="one-time-code" enterKeyHint="done" onSubmit={submit} className={field} />
      <radio bind={plan} accessibilityLabel="Plan" className="gap-1 text-sm"
        options={[{ value: 'basic', label: 'Basic' }, { value: 'pro', label: 'Pro' }]} />
      <slider bind={seats} min={1} max={20} step={1} accessibilityLabel="Seats" className="text-accent" />
      <switch bind={news} className="text-sm">Product news</switch>
      <button disabled={busy.value || !email.value} onClick={submit}
        className="rounded-lg bg-accent text-on-accent disabled:bg-line disabled:text-muted">
        {busy.value ? 'Signing up…' : `Sign up for ${seats.value} seats`}
      </button>
    </page>
  );
}
```

## Binding

Every control is controlled. Either bind a `useLocal` cell of the right type:

```tsx
import { useLocal } from '@lingxia/ui';

export function render() {
  const name = useLocal('name', '');
  return <page className="p-4"><input bind={name} accessibilityLabel="Name" /></page>;
}
```

or pass `value` and the change callback, and store the value yourself:

| Element | Value | Callback |
| --- | --- | --- |
| `<input>`, `<textarea>`, `<search>` | `string` | `onInput(text)` |
| `<checkbox>`, `<switch>` | `boolean` | `onChange(checked)` |
| `<radio>` | `string` (an option's `value`) | `onChange(value)` |
| `<slider>` | `number` | `onChange(value)` |

`bind` with `value` or the callback is refused. A `bind` cell must be mounted in the current render and
hold the control's type (`UI_INPUT`, `UI_CHECK`, `UI_RADIO`, `UI_SLIDER`).

`onInput` receives committed text only. While an input method is composing (pinyin, kana, a
swipe-typed word), the editor shows the composition but `onInput` waits until it is committed.

## Editors

| Prop | Applies to | Effect |
| --- | --- | --- |
| `placeholder` | all | hint text; color it with `placeholder:text-*` |
| `secure` | `input`, `textarea` | masked text, no learning |
| `keyboard` | `input`, `textarea` | `text`, `number`, `email`, `phone`; `date`, `time` on `<input>` only |
| `maxLength` | all | 1–65536 UTF-16 units; insertions are cut, existing text is kept |
| `autoComplete` | all | HTML autocomplete token, mapped to platform autofill (`email`, `one-time-code`, `new-password`, …) |
| `enterKeyHint` | all | the label of the keyboard's action key |
| `autoCapitalize` | all | `none`, `sentences`, `words`, `characters` (`off`/`on` are `none`/`sentences`) |
| `spellCheck` | all | spelling suggestions on or off |
| `autoFocus` | all | focus when it first appears |
| `submitOnEnter` | `textarea` | Enter submits; Shift+Enter inserts a newline |
| `onSubmit(text)` | all | the keyboard's action key, or Enter on a single-line editor |

Every action key calls `onSubmit`: `enterKeyHint` only chooses its label, and focus does not move to the
next field on its own. There is no `focus()` API; `autoFocus` is the only way to place focus.

`<search>` is an `<input>` with the search keyboard and room for its magnifier; it takes no `keyboard`
or `secure`. `<textarea>` is 120 px tall until sized.

`focus:` styles a focused editor (paint only). Focus follows the editor across reorders and ends when it
is disabled or removed. `userSelect` (`select-*`) is refused on editors: they always select their own text.

## Numbers, dates and times

With `keyboard="number"`, `min`, `max` and `step` clamp and snap the text when the editor blurs or
submits, and the corrected text is reported through `onInput`. The value stays a string; parse it yourself.

With `keyboard="date"` or `keyboard="time"`, tapping the input opens the platform picker instead of a
keyboard. The value is ISO: `YYYY-MM-DD`, or `HH:MM` / `HH:MM:SS`. `min` and `max` take the same form.

```tsx
import { useLocal } from '@lingxia/ui';

export function render() {
  const day = useLocal('day', '');
  const amount = useLocal('amount', '');
  return (
    <page className="p-4 gap-2">
      <input bind={day} keyboard="date" min="2024-01-01" accessibilityLabel="Start date" placeholder="Start date" />
      <input bind={amount} keyboard="number" min={1} max={500} step={0.5} accessibilityLabel="Amount" placeholder="Amount" />
    </page>
  );
}
```

`min`/`max`/`step` without one of these keyboards, or `step` on a date, is refused:

```tsx error UI_INPUT
export function render() {
  return <page><input keyboard="text" min={1} accessibilityLabel="Count" /></page>;
}
```

## Checkbox and switch

`<checkbox>` may contain noninteractive content (text, icons, images, layout) that becomes part of the
control and its accessible name. `<switch>` takes plain text as its label. Both accept
`accessibilityLabel` when they have no visible text.

## Radio

`<radio options={[{ value, label, disabled?, testId? }]}>` draws a group of 1–64 options. Values are
unique non-empty strings. Choosing the selected option again does nothing. `disabled` on the group
disables every option.

## Slider

`<slider>` takes `min` (0), `max` (100) and `step` (1); the value snaps to the step. It must have an
`accessibilityLabel`. `text-*` sets its accent color.

## Forms

There is no `<form>` element. Keep the form state in `useLocal` cells, validate in TypeScript, call a
Logic action to submit, and disable the submit button while the action runs (as in the first example).
`disabled:` styles replace the button's whole class cascade while it is disabled.
