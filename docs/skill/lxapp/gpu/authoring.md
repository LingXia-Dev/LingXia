# GPU View authoring

The default View (Android, iOS, macOS, HarmonyOS, Windows). Styling, theme and motion:
[styling-motion.md](./styling-motion.md); tests: [testing.md](./testing.md).

A page whose View (`index.tsx`, Logic stays in `index.ts`) exports a named `render` renders natively;
one that exports a default component is a WebView page. All pages of an lxapp must agree, or the
build stops. Add `@lingxia/view` as a dependency.
There is no DOM. The host owns navbar and tabbar. Build controls, rows, fields and sheets from
Liuli (`@liulidesign/core`) before composing primitives: `lingxia ui search <what the user sees>`, then `lingxia ui info <Part>`.

```tsx
import { useLocal, useRequest } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const draft = useLocal('');
  const item = useRequest('item', actions.load, { id: data.id });
  return <page className="p-4 gap-3">
    <text className="text-lg font-semibold truncate">{item.value?.title ?? 'Loading…'}</text>
    <input bind={draft} aria-label="Note" className="px-3 py-2 rounded-lg border" />
    <button disabled={!draft.value} onClick={() => draft.set('')} className="rounded-lg bg-accent-solid px-4 py-2 text-on-accent-solid active:opacity-80">Clear</button>
  </page>;
}
```

## Types and build

- `./$types` types `data` and `actions` from the page's Logic (`index.ts`); a page without Logic gets empty ones. `lingxia dev`, `build` and `check` write it under `.lingxia/types` (run `lingxia check` on a fresh checkout or in CI).
- `tsconfig.logic.json` / `tsconfig.view.json` extend the CLI's `./.lingxia/tsconfig.logic.json` / `.view.json`, the View referencing the Logic project. Type-check with `lingxia check` (or `tsc -b` at the project root); `tsc -p` alone cannot build the Logic types the View reads. `lingxia build` fails a `./$types` page whose View tsconfig skips the base.
- In `lingxia dev`, saving View code of GPU pages swaps those pages in place, keeping `useLocal`, scroll
  and focus; Logic or config edits rebuild and reopen. Dev builds' runtime errors name `file:line:col`.
- `console.log`/`info`/`warn`/`error`/`debug` in View code go to the app log tagged with the page (`lxdev logs`); arguments join as text and a call never throws.
- The build reports every problem with `file:line:col`, split into unknown
  utilities, ones the renderer cannot draw, and ones the element ignores.
- JSX evaluated at module scope fails the build (`UI_SCOPE`): build it inside the component, or make it a function.
- `UI_COMPAT` means the installed host is older than the bundle: update it.
- `UI_PERF` build warnings do not stop the build: use `<List>` past 50 literal rows, keep opacity and
  backdrop off repeated rows, and ship bundled images at most 2048 px.

## Layout

- `<page>` is the root: it absorbs safe-area insets and the keyboard. `<view>` is the only layout container: a flex column; `flex-row`/`flex-col` set direction, `flex` alone means row, `grid` makes a grid; `gap`/`items`/`justify` work without `flex`. Overlap children with `grid *:col-start-1 *:row-start-1`.
  A last-child `<scroll>`/`<List>` runs on under the tab bar, its end padded clear; `<dialog>`s cover the bars.
  `fixed` insets and `vh`/`dvh` measure the page's safe box (inside system bars, above the keyboard);
  `hidden` removes a node entirely.
- Layout is flex or CSS Grid (`grid grid-cols-[repeat(auto-fit,minmax(8rem,1fr))] col-span-2`).
  Row items shrink to min-content as in CSS (`min-w-0` to truncate, `shrink-0` to pin). `<button>` is
  unstyled as after Tailwind's preflight: give it its fill, padding and radius.
  `w-`/`min-w-`/`max-w-[10ch]` measure the element's own `0`; with `tabular-nums` that is ten digits.
- Lay out from the View's `env` (`viewport`, `sizeClass`, `insets`); `page.env.watch` is for Logic that acts
  on a size change itself, never to forward it with `page.update`. `env` also carries `keyboard.height`,
  `capsule` (the floating ··· control's box, or `null`), `reducedMotion`, `hourCycle` and `language`;
  `os` is for shortcut symbols only, never layout.

## State and handlers

- `useLocal(value)` state is keyed by call order and component identity; `useLocal(name, value)` by name (survives reorders and hot edits).
- No effects: a handler acts, also after an `await` — `if (!(await signIn.run(form)).ok) password.focus()` (or `.set`, `reveal`). Its closure keeps the old render's `data`, so let the action's result answer, not `data` read after the await.
- A handler that throws or rejects fails the page in a `lingxia dev` build and under test; a shipped app logs it (`UI_HANDLER: onClick of <button data-testid=…> rejected: …`) and the page stays. A sync throw discards that turn's writes; an async handler's writes made before it failed stay. Prefer `useAction(...).run`, which answers `{ ok: false }`; bare `await actions.x()` rejects — handle it or it is reported.
- `onClick(event)`, `<video>` and `onVisibleChange` events carry `event.timeStamp`, epoch ms when the host took the event: stamp observations with it, not `Date.now()`.
- `useNow('second' | 'minute')` (from `@lingxia/view`) is the wall clock in epoch ms, floored to that step; only what reads it re-renders, on the boundary. The host ticks it only while the page is shown, never wakes Logic, re-delivers at once on resume or a clock/time-zone change. For clocks and relative times; never `Date.now()` in render.
- In a component, `useEnv()` re-runs it only when an env field it read changes, not on every `view.data` change (`useView()` does).
- `cell.focus()` (text cells only, `LocalText`) focuses the editor bound (`bind`) to that cell; from a handler, never in render.
  `cell.showPicker()` opens a bound date or time editor's picker the same way (nothing if disabled or read-only).
  `cell.setSelection(start, end)` and `cell.replaceRange(start, end, text)` (UTF-16 offsets) need `bind` the same way; an unfocused editor takes the range at its next focus. `onSelectionChange={({ start, end, caret, rects }) => …}` on the editor hears every move, at most once a frame; `caret` and `rects` are in page points (`caret` null while scrolled out), for custom drawing; anchor a menu with `:caret` instead. A value set from code keeps the caret beside its text.
  `onPaste={event => …}` hears a paste before anything is inserted: `text` raw, `html` as the source app wrote it (unsanitised; absent for a plain paste or a secure editor). Call `event.preventDefault()` synchronously, then insert your own result with `replaceRange(event.selection.start, event.selection.end, …)`; otherwise the plain text goes in. `files` stays empty for now. Test with `screen.paste(node, { text, html })`.
- `<button keepsFocus>` (a format bar's): its press leaves the focused editor editing, caret and keyboard kept.
- `onVisibleChange={({ items, current }) => …}` on `<scroll>`/`<List>` lists the children on screen (`id`'d elements; List rows by key) with `ratio` and `top`; it fires on a set, `current` or quarter-ratio change while moving, and once exactly at rest. Don't derive it from `onPointer`.
- `reveal(id)` (from `@lingxia/view`, in a handler) scrolls the element with that `id` into view inside its `<scroll>`s, after the render the handler causes; a `<List>` row uses the list's `anchor`.
- `const box = useBox(name)` + `<view box={box}>` reads the element's laid-out content size: `box.value` is `{ width, height, minContentWidth, maxContentWidth }` in points (the last two: its content at its narrowest and on one line, to choose a layout that fits), `null` until the first layout and one render behind a change (`onLayout={size => …}` is the callback form; on `view`, `text`, `vector`, `plot`, `scroll`).
- `useAction(actions.x)` gives `pending`, `error` and `run`, which never rejects and resolves `{ ok: true, value }` (`value` is `null` for an action that returns nothing) or `{ ok: false, error }`.
- `useStream` keeps the latest yielded value in `data`; `onData` is for side effects.
- `useRequest`, `useStream(name, action, { onData? })`,
  `useChannel(name, action, { params, onData? })` take `view.actions` references.
  Errors are `{ code, message }`; reloads keep old values (`stale`). `useLeaveGuard(name, { enabled })`.
- Lists: `<List items getKey renderItem />`; hooks inside rows need stable keys. `onReachEnd`/`onReachStart`
  on a `<List>` or `<scroll>` page more items in; `anchor="end"` starts at the last row (a chat).
- `cx(...)` joins class names (the build still checks each class); `useId()` makes the ids `popoverTarget`,
  `anchor` and `aria-actions` name, unique per row; `createContext`/`useContext` pass values down.
- A component taking a child as its trigger: `elementType(child)` tells what it is; `cloneElement(child, props)` adds props in its place (`className` joins; replacing a handler it has needs `{ override: ['onClick'] }`).
- A component re-runs only when its props, the `useLocal` cells it reads or the environment change, so render must read
  nothing else; one that reads `Date`, `Math.random` or a module `let` re-runs every render.

## Pointer and gestures

Drag, swipe rows, sheet detents and scroll snap: [Drag and snap](./styling-motion.md#drag-and-snap).

- `onPointer={e => …}` on `view`, `button`, `vector` reports a pointer over the element: `e.phase` (`down`/`move`/`up`/`cancel`, and `hover`/`leave` for a mouse), `e.x`/`e.y` in points from its content box's top-left (not clamped), `e.width`/`e.height`. It observes: buttons inside still click. `pointerAxis="x"` leaves vertical movement to the page's scroll (then `cancel`). On a `<view>` with `aria-valuenow`/`-valuemax`/`-valuetext` it is a keyboard stop: arrows and a screen reader's adjust send `phase` `increment`/`decrement`.
- `onPointerEnter`/`onPointerLeave` on `view`/`button` follow a mouse. Beyond these, `drag` and `onContextMenu`, there are no swipe, pinch or long-press handlers and no drag-and-drop.
- On phones a finger reaches a control smaller than 44 pt (Android 48 dp) within that size around it, never over
  another control; layout, `boundingBox()` and mouse input keep the real box. `touch-target-none` opts out.
- `onContextMenu={e => …}` on `view`/`button`/`link`/`text`: long press, secondary click or `locator.performAction('contextmenu')`; `e = { x, y, pointer }` in page coordinates. The engine draws no menu.
- A press that misses every box goes to the nearest small control within the platform touch target (44 pt, 48 dp on Android); `locator.touchTarget()` reads that box.

## Text, links and semantics

- Rich text: `<text>Read <span className="font-bold">this</span> or <link onClick={go}>that</link></text>`;
  runs take text paint only; links are accent + underline by default. `<link to={{ page: 'detail', params }}>`
  navigates without an action: `to` takes `lx.nav.go` targets or `{ back: n }`, type-checked against the page registry (`replace` swaps the current page); a link to the current page does nothing, and a refused target reports like a failed action. `decoration-dotted`/`-dashed` and `decoration-<role>/<alpha>` style an underline or strike. `select-text` lets users
  long-press to select the nearest word and copy; there a long-press on a link selects, elsewhere it presses.
- Outside `<text>`, `<link>` is a block link: a card of any content with `<button>`'s classes, role link; a button inside keeps its own press, Enter opens it, Space does not. Block links do not nest; every `<link>` needs `to`, `onClick`, `popoverTarget` or `interestFor` (`UI_LINK`).
- Semantics: `role="heading" headingLevel={2}`, `list`/`listitem`, `menu`/`menuitem`, `alert`/`status`
  (announced), `aria-hidden` for decoration, `role="img" aria-label` on a drawn picture (avatar, rating) speaks only the label. A layout element with a role, or a scroll,
  groups its content for screen readers ("item 2 of 3"); plain wrappers collapse.
  `aria-pressed`, `aria-selected`, `aria-expanded`, `aria-busy` announce states (e.g. `aria-busy={req.pending}`).
- `role="grid"` > `role="row"` > `role="gridcell"`: one Tab stop, arrows by row and column, Home/End to the row's ends; `onPage("previous" | "next")` on the grid takes PageUp/PageDown.
- Buttons may contain text, icons and images; their text names them for
  accessibility. Images need `alt` (`""` for decoration).
- The build fails with `UI_A11Y` when an editor, slider, radio group, select, plot, dialog or `<webview>` lacks `aria-label`,
  a button, link, checkbox or switch has neither text nor a label, or an `<image>` lacks `alt`; a placeholder is not a name.
- Text follows the system font size, up to 2× by default: lxapp.json `maxTextScale` (1 to 3) sets the cap and a
  page's json may lower it; `env.textScale` reads the applied factor (1 at default) for a layout that must change
  shape. Prefer `min-h-*` over fixed heights. Whitespace
  collapses as in CSS: use `whitespace-pre-line`/`pre-wrap` for authored newlines.

## Dialogs, popovers and overlays

- `<dialog open onClose closedby aria-label>` is modal, above the page (`backdrop:` styles its scrim;
  `mt-auto mb-0 w-full` makes a sheet). A panel docked to an edge (`mb-0 w-full`, `ms-0 h-full`) runs to the screen's
  edge and its padding grows by that side's inset: never add `env.insets` yourself. `<view id popover>` + `<button popoverTarget>` is a light-dismiss
  popover anchored to its invoker (`open`/`onToggle`; `style={{ positionArea: 'top' }}` opens it upward; it closes if that button unmounts; `anchor="<id>"` places one opened from state against any element, e.g. a `popover="manual"` list under a field; `anchor="<editor-id>:caret"` follows that editor's caret each frame, `:selection` the box around its selection, as a mention list or a desktop bubble bar does, falling back to the editor while it is unfocused); `role="dialog"` on a popover is a non-modal dialog (`aria-modal` makes it modal); `role="menu"` + `<button role="menuitem">` closes on click (`keepOpen` leaves it open); a non-`manual` popover's `onCloseRequest` returning `false` refuses Escape/back, never a tap outside; `popover="manual" closeAfter={ms}` is a toast; `backdrop:bg-scrim` dims the page under a light-dismiss popover, as a docked menu sheet does. A popover flips to the other side when it does not fit; `side-top:`/`side-bottom:`/`side-start:`/`side-end:` (paint only) style it and its contents by the side it ended on, so a caret inside it can face the anchor; an absolute caret with `start-[anchor(center)]` (or `top-[anchor(center)]`) sits on the anchor's centre line even when the popover is pushed in or aligned to an edge, kept off its rounded corners.
- Leaving a page inside the app (push, tab switch, closing its surface) closes what a tap outside would (the select/date picker, light-dismiss popovers, menus, hints, an action sheet); dialogs and `manual` popovers stay. Going to the background closes nothing.
- In-page overlays are `<dialog>`/popovers; `lx.surface.openPage(page, { as: 'float' })` is for a surface that
  stays across pages (a mini player), and it always sits above the page and its dialogs.
- `<webview src aria-label>` embeds a web document in the page; pass one `useWebView(name)` handle as `web` to each `<webview>` showing it (inline and in a `<dialog>`) to keep one live document. `onMessage` receives what the web page posts with `window.lingxia.postMessage`. A browser of its own is `lx.surface.openUrl(url, { as: 'main' | 'aside' })`.
- Keyboard: Tab and arrows follow reading order (no `tabIndex`); an open `<dialog>` keeps focus inside
  and returns it on close; the focus ring shows only after keyboard input. In a menu, arrows wrap,
  Home/End jump, a typed letter finds the next item; `expect(locator).toBeFocused()` checks focus.

## Inputs

- An editor takes `bind={cell}` or `value` + `onInput` (checkbox, switch, select, radio: `value` + `onChange`),
  never both. `onSubmit(value)` runs on the keyboard's action key; `autoFocus` focuses it on first show.
- `<radio options bind aria-label>` is a radio group; `<plot kind values colors aria-label>` a bar, line or pie
  chart (`onSelect`/`selectedIndex`), though Liuli charts come first.
- Inputs take `type` (`password`, `email`, `number`, `tel`, `search`, `date`, `time`), `inputMode`, `maxLength`, `autoComplete`, `enterKeyHint` (on a textarea, `send`/`go`/`done`/`search` makes Return submit), `autoCapitalize`, `spellCheck`;
  `type="number"` with `min`/`max`/`step` clamps on blur/submit, keeping the typed text otherwise (`7.0` stays);
  typed and pasted text is filtered by keyboard: `inputMode="numeric"` keeps digits, `type="number"`/`inputMode="decimal"`
  also one `.` (a `,` becomes `.`) and a leading `-`; `type="date|time"` opens the
  View's own themed picker (popover with a mouse, sheet on touch) and reports ISO strings;
  a time field shows the user's 12/24-hour clock, and both size to fit any value, so drop `min-w-[…ch]` on them.
- `<textarea rows={2} maxRows={6}/>` grows with its lines in that range, then scrolls.
- `truncate` on an `<input>` ends its placeholder, and its value while unfocused, in an ellipsis; a focused editor scrolls.
- `onFocus`/`onBlur` on `<input>` and `<textarea>` fire as the caret enters or leaves; a press on a button does not blur the editor. `readOnly` keeps focus and selection, takes no edits and raises no keyboard. `onKeyDown={{ ArrowDown: fn, Enter: fn, … }}` gives the page the listed keys (ArrowUp/Down, Enter, Escape, Home, End, PageUp/Down) while the caret stays; listed means consumed. `aria-activedescendant` on the editor names the option those keys point at; `role="combobox"` (input or button, with `aria-valuetext`) and `role="spinbutton"` (number input with `aria-valuenow`) are accepted, and `inputMode` `numeric`/`decimal` may sit beside `type="number"`.
- `<select options={[{value,label,disabled?}]} bind={cell} placeholder aria-label/>` opens the View's own
  picker (popover with a mouse, sheet on touch); arrows change the choice; automation uses `locator.selectOption(valueOrLabel)`.
- `<slider values={[lo, hi]} onChange thumbLabels={['From','To']} aria-label>` has two thumbs that cannot cross (no `bind`); `onChangeEnd` runs when a change is finished (the press lifts, or a key step), on a one-value slider too.
- A `<checkbox>`, `<switch>`, `<slider>` or `<select>` with no `value`/`bind` keeps its own state across re-renders and still calls `onChange(next)`; an unbound slider starts at `min`, a select at its placeholder, unless `defaultChecked`/`defaultValue` seeds it once; `has-checked:` and a switch's `checked:track:`/`checked:thumb:` follow the state either way.
- `<checkbox indeterminate>` is partly checked: a dash, announced as mixed, `indeterminate:` variant; a press calls `onChange(true)`. Automation reads `checked: "mixed"`.
- `enterKeyHint="next"|"previous"` moves focus to the adjacent enabled editor in reading order
  (the last one submits).

## Images, vectors and video

- `<vector paths gradients>`: paths take SVG `transform`, caps, joins, dash offset and
  `fill`/`stroke: 'url(#id)'` naming a `gradients` entry (linear, radial or conic, ≤8).
- `<image>` `onLoad`/`onError` fire once per `src`; changing `retryKey` loads it again after an error.
- While an image loads its background colour shows; `placeholder="tiny.jpg"` paints a small stand-in scaled smooth.
- `object-top`, `object-left-bottom` or `object-[25%_75%]` (`objectPosition`) align a covered or contained image; pixel offsets fail.
- Remote `<image>` sources follow HTTP caching: revalidated, and shown stale while offline.
- `<video>` HLS at quality `auto` never fetches a rendition larger than the picture shows (HarmonyOS: the system player
  chooses); `setQuality(label)` pins past it. No picture 15 s after play fails as `VIDEO_NETWORK`.
- `<video>` `paused`, `muted`, `loop`, `playbackRate` and `volume` apply on mount and when they change, not on
  every render: re-rendering the same value does not undo a handle's `play()`; act through the handle.
- Drive a GPU page's video from Logic with `page.video(name)` (the `page`
  `setup` received), by its `useVideo` name or `<video id>`:
  `play`, `pause`, `stop`, `seek(ms)`, `setQuality`, `setPlaybackRate`,
  `setMuted`, `setVolume`. Each returns a `Promise<void>` that rejects with a
  `VIDEO_*` code; fullscreen and `setStreamSource` reject `VIDEO_UNSUPPORTED`.
