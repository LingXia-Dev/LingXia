# Inputs and forms

```tsx
import { useLocal, type View } from '@lingxia/view';

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
      <input bind={email} aria-label="Email" placeholder="you@example.com" type="email"
        autoComplete="email" autoCapitalize="none" enterKeyHint="next" className={field} />
      <input bind={code} aria-label="Invite code" placeholder="6 digits" inputMode="numeric"
        maxLength={6} autoComplete="one-time-code" enterKeyHint="done" onSubmit={submit} className={field} />
      <radio bind={plan} aria-label="Plan" className="gap-1 text-sm"
        options={[{ value: 'basic', label: 'Basic' }, { value: 'pro', label: 'Pro' }]} />
      <slider bind={seats} min={1} max={20} step={1} aria-label="Seats" className="text-accent" />
      <switch bind={news} className="text-sm">Product news</switch>
      <button disabled={busy.value || !email.value} onClick={submit}
        className="rounded-lg bg-accent-solid p-3 text-on-accent-solid disabled:bg-line disabled:text-muted">
        {busy.value ? 'Signing up…' : `Sign up for ${seats.value} seats`}
      </button>
    </page>
  );
}
```

## Binding

Every control is controlled. Either bind a `useLocal` cell of the right type:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const name = useLocal('name', '');
  return <page className="p-4"><input bind={name} aria-label="Name" /></page>;
}
```

or pass `value` and the change callback, and store the value yourself:

| Element | Value | Callback |
| --- | --- | --- |
| `<input>`, `<textarea>` | `string` | `onInput(text)` |
| `<checkbox>`, `<switch>` | `boolean` | `onChange(checked)` |
| `<radio>`, `<select>` | `string` (an option's `value`) | `onChange(value)` |
| `<slider>` | `number` | `onChange(value)` |

A `<checkbox>`, `<switch>`, `<slider>` or `<select>` with neither keeps its own state and still reports each change
to its callbacks. It starts at `defaultChecked` or `defaultValue`, applied once, else unchecked, at `min` or with
nothing chosen. `checked:` and `has-checked:` styles follow its state either way.

`<scroll>` snapping and `drag` take `bind` only (a `number` cell). `bind` with `value` or the callback is refused. A `bind` cell must be mounted in the current render and
hold the control's type (`UI_INPUT`, `UI_CHECK`, `UI_RADIO`, `UI_SLIDER`).

`onInput` receives every value change, as DOM `input` events do, including while an input method
is composing (pinyin, kana, a swipe-typed word, or any word on keyboards that compose as you type):
the value holds the composed text and the composition stays open for the input method. Leaving the
editor finishes the composition, so the last composed text is the value.

## Editors

| Prop | Applies to | Effect |
| --- | --- | --- |
| `placeholder` | all | hint text; color it with `placeholder:text-*` |
| `type` | `input` | `text`, `password` (masked, no learning), `email`, `number` (a keyboard with a decimal point; `inputMode="numeric"` for digits only), `tel`, `search`, `date`, `time` |
| `inputMode` | `input`, `textarea` | keyboard only: `text`, `numeric` (digits), `decimal` (digits and the locale's separator), `tel`, `email`; with `type="number"` only `numeric`/`decimal`, not with `search`/`date`/`time` |
| `maxLength` | all | 1–65536 UTF-16 units; insertions are cut, existing text is kept |
| `autoComplete` | all | HTML autocomplete token, mapped to platform autofill (`email`, `one-time-code`, `new-password`, …) |
| `enterKeyHint` | all | the label of the keyboard's action key |
| `autoCapitalize` | all | `none`, `sentences`, `words`, `characters` (`off`/`on` are `none`/`sentences`) |
| `spellCheck` | all | spelling suggestions on or off |
| `autoFocus` | all | focus when it first appears |
| `onSubmit(text)` | all | the keyboard's action key, or Enter on a single-line editor |
| `readOnly` | all | takes focus and a selection, so its text can be copied; takes no edits and raises no keyboard, and a date or time field opens no picker |
| `onKeyDown={{ ArrowDown, ArrowUp, Enter, Escape, Home, End, PageUp, PageDown }}` | all | the listed keys call the page while the caret stays in the text, and do nothing else (no caret move, newline or submit); other keys stay with the editor, and an input method that is composing keeps them all. `Escape` left out closes the topmost overlay |
| `onFocus()`, `onBlur()` | all | the caret enters or leaves the editor; pressing a button does not blur it, moving to another editor blurs it first |

Every action key calls `onSubmit`: `enterKeyHint` chooses its label, and focus does not move to the
next field on its own. On a `<textarea>`, `enterKeyHint` `send`, `go`, `done` or `search` makes Return
submit (Shift+Return inserts a newline); otherwise Return inserts a newline. `autoFocus` places focus on mount; `cell.focus()` focuses the editor bound to that `useLocal` cell.

`<input type="search">` has the search keyboard and room for its magnifier. `<textarea>` is 120 px tall until sized.

`focus:` styles a focused editor (paint only). Focus follows the editor across reorders and ends when it
is disabled or removed. `userSelect` (`select-*`) is refused on editors: they always select their own text.

## Numbers, dates and times

With `type="number"`, `min`, `max` and `step` clamp and snap the text when the editor blurs or
submits, and the corrected text is reported through `onInput`. The value stays a string; parse it yourself.

With `type="date"` or `type="time"`, tapping the input opens the View's own picker instead of a
keyboard, drawn in the theme's roles and the same on every platform: a popover under the field with a
mouse or trackpad, on touch a bottom sheet with Cancel, a title and Done along its top, where a tap picks and Done commits. The date picker shows a month (weeks
start on `env.firstWeekday`), marks today and the value, and greys days outside `min`/`max`; arrows move
by day and week, Page Up/Down by month, Return picks, Escape cancels. The time picker has hour and minute
columns (and AM/PM when `env.hourCycle` is `h12`) that snap as they scroll; `step` (seconds) sets the
minutes offered, and typed digits fill `HH:MM`. Month and day names follow `env.displayLanguage`.
Choosing runs `onInput` then `onSubmit`, as typing and Return would. The value is ISO: `YYYY-MM-DD`, or
`HH:MM` / `HH:MM:SS`. `min` and `max` take the same form; automation can `fill` the field directly.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const day = useLocal('day', '');
  const amount = useLocal('amount', '');
  return (
    <page className="p-4 gap-2">
      <input bind={day} type="date" min="2024-01-01" aria-label="Start date" placeholder="Start date" />
      <input bind={amount} type="number" min={1} max={500} step={0.5} aria-label="Amount" placeholder="Amount" />
    </page>
  );
}
```

`min`/`max`/`step` without one of these types, or `step` on a date, is refused:

```tsx error UI_INPUT
export function render() {
  return <page><input type="text" min={1} aria-label="Count" /></page>;
}
```

## Checkbox and switch

`<checkbox>` may contain noninteractive content (text, icons, images, layout) that becomes part of the
control and its accessible name; with content it draws no box of its own, so draw the checked state from its value. `<switch>` takes plain text as its label. Both accept
`aria-label` when they have no visible text.

A switch's `track:` and `thumb:` parts restyle it; `checked:` before a part applies while it is on, and
state changes ease over 150 ms. The thumb rests evenly inside the track, and the label makes room for the track's width:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const wifi = useLocal('wifi', true);
  return (
    <switch bind={wifi} className="track:w-11 track:h-6 checked:track:bg-success thumb:size-5 active:thumb:w-6">
      Wi-Fi
    </switch>
  );
}
```

A disabled switch or checkbox that is on keeps its accent at reduced strength. To centre a composed switch's
or checkbox's content, use `my-auto` or `self-center` on it, not `grow`: `grow` takes the free space first.

A checkbox that stands for a set which is partly chosen takes `indeterminate`: it draws a dash, a
screen reader hears "mixed", and `indeterminate:` styles it. A press calls `onChange(true)`; clear
`indeterminate` yourself when the set becomes all or none. A `menuitemcheckbox` item says the same
with `aria-checked="mixed"`.

## Radio

`<radio options={[{ value, label, disabled?, 'data-testid'? }]}>` draws a group of 1–64 options. Values are
unique non-empty strings. Choosing the selected option again does nothing. `disabled` on the group
disables every option.

## Select

`<select options={[{ value, label, disabled? }]}>` takes 1–64 options and opens the View's own picker, like the date and time
inputs: a listbox popover with a mouse or trackpad, a bottom sheet on touch. Arrows change the choice,
typing jumps to a label; automation uses `locator.selectOption(valueOrLabel)`.

## Slider

`<slider>` takes `min` (0), `max` (100) and `step` (1); the value snaps to the step. It must have an
`aria-label`; `aria-valuetext` says how the value reads aloud (`"1:20 of 3:05"`). `text-*` sets its
accent color. `secondaryValue` draws a second range behind the value, such as buffered media.

Style its parts with `track:`, `range:`, `secondary:` and `thumb:`, after a state when needed:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const at = useLocal('at', 80);
  return (
    <slider bind={at} max={185} secondaryValue={120} aria-label="Seek" aria-valuetext="1:20 of 3:05"
      className="track:h-[3px] active:track:h-[5px] thumb:size-0 active:thumb:size-3 focus-visible:thumb:size-3 focus-visible:thumb:ring-2" />
  );
}
```

State changes on parts ease over 150 ms; the thumb follows the value directly. The touch target is the
slider's own box and never shrinks with the thumb's look, so a `thumb:size-0` scrubber stays fully draggable.
The track ends sit inside the padding by half the thumb's widest state.
The thumb's centre therefore travels less than the box is wide: a press at 90 % of the box is past 90 % of the
range, and may land on the last step.

`onChangeEnd(value)` runs once a change is finished: when the finger or mouse that made it lifts, or at
once for an arrow key or a screen reader step. Save or fetch there, and keep `onChange` for what follows the thumb.

### Two thumbs

`values={[lower, upper]}` makes a range with two thumbs. A press moves the nearer thumb, the thumbs stop
at each other, and `onChange` and `onChangeEnd` take `[lower, upper]`:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const price = useLocal<readonly [number, number]>('price', [20, 60]);
  return (
    <slider values={price.value} onChange={price.set} step={5} aria-label="Price" thumbLabels={['From', 'To']}
      className="w-full text-accent thumb:size-5" />
  );
}
```

- `aria-label` names the pair and `thumbLabels` each thumb: a screen reader meets two sliders in a group,
  and Tab stops on each. The arrow keys move the focused thumb.
- Classes that place and size the control (`w-*`, margins, `h-*`, a background) apply to the pair as one box;
  parts, states, padding and the accent color apply to both thumbs.
- It takes `values` and `onChange`, not `bind`, `value`, `secondaryValue` or `aria-valuetext`.
- It needs a host that knows it; on an older one the page fails with `UI_COMPAT`.

## Forms

There is no `<form>` element. Keep the form state in `useLocal` cells, validate in TypeScript, call a
Logic action to submit, and disable the submit button while the action runs (as in the first example).
`disabled:` styles replace the button's whole class cascade while it is disabled.

## Disclosure

HTML's details/summary is a `<button>` with `aria-expanded`, which a screen reader announces as
expanded or collapsed, and a body that exists only while open. Keep the open state in `useLocal`.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const open = useLocal(true);
  return (
    <page className="p-4 gap-2">
      <button aria-expanded={open.value} onClick={() => open.set(!open.value)} className="py-2">
        <text className="text-sm font-semibold">Open-source licenses</text>
      </button>
      {open.value && <text className="text-sm text-muted">MIT, Apache-2.0</text>}
    </page>
  );
}
```
