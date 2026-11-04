# GPU Views (Android, iOS, macOS)

An lxapp renders natively when its `lxapp.json` has `"renderer": "gpu"`: every page does, and a page
config naming a renderer fails the build (no mixed apps). A page's View is `index.tsx` (Logic stays in
`index.ts`). Add `@lingxia/view` as a dependency.
There is no DOM. The host owns navbar and tabbar.

```tsx
import { useLocal, useRequest, type View } from '@lingxia/view';

type Actions = { load(q: { id: string }): Promise<{ title: string }> };

export function render({ data, actions }: View<Actions, { id: string }>) {
  const draft = useLocal('');
  const item = useRequest('item', actions.load, { id: data.id });
  return <page className="p-4 gap-3">
    <text className="text-lg font-semibold truncate">{item.value?.title ?? 'Loading…'}</text>
    <input bind={draft} aria-label="Note" className="px-3 py-2 rounded-lg border" />
    <button disabled={!draft.value} onClick={() => draft.set('')} className="rounded-lg bg-accent px-4 py-2 text-on-accent active:opacity-80">Clear</button>
  </page>;
}
```

- `<page>` is the root: it absorbs safe-area insets and the keyboard. `view` is a column until `flex` makes it a row, as in CSS.
  `fixed` insets and `vh`/`dvh` measure the page's safe box (inside system bars, above the keyboard);
  `hidden` removes a node entirely.
- Layout is flex or CSS Grid (`grid grid-cols-[repeat(auto-fit,minmax(8rem,1fr))] col-span-2`).
  Row items shrink to min-content as in CSS (`min-w-0` to truncate, `shrink-0` to pin). `<button>` is
  unstyled as after Tailwind's preflight: give it its fill, padding and radius.
- Style with complete Tailwind classes. `style` (CSS-named: `backgroundColor`, `marginTop`,
  `gridTemplateColumns`) is only for values computed at render time; a `style` object of fixed values
  warns `UI_STYLE_STATIC`. Unitless `lineHeight` is a multiplier. Each element accepts only styles it renders.
- RTL: use `ps`/`ms`/`start-`/`text-start` etc.; they mirror with `env.direction` (from the
  display language) or a `dir` prop. Mirror icons with `rtl:-scale-x-100`.
- Colours are theme roles, never hex: `bg-surface`, `text-muted`, `border-line` (decorative) or
  `border-line-strong` (editors, controls), `bg-accent text-on-accent`, `bg-text/8`; plot series
  `chart-1`..`chart-5`. Roles follow appearance, so `dark:` on a colour fails (`UI_COLOR`).
- `lingxia gen tokens` writes the theme pack's type, radius and spacing scales to `theme/tokens.css`.
- `theme/pack.json` gives the lxapp its own palette; it may set only `accent` (`{"seed":"#…","derive":true}`),
  the rest comes from the default pack. A palette the host pushes wins. `env.highContrast` picks the
  pack's high-contrast palette; `shadow-*` layers use the `shadow` role.
- Supported variants: `dark:`, `rtl:`/`ltr:`, `first:`/`last:`/`odd:`/`even:`/`only:`/`empty:`, `sm:`–`2xl:`, `focus:` (editors), `active:`
  (controls), `hover:` (fine pointers), `disabled:`, `placeholder:`,
  `space-*`/`divide-*`. `data-*`, `has-*`, `focus-within:`, `group-active:`. `*:`, `**:`, `in-*:`, other `group-*:`, `peer-*:` fail naming the TS-side form.
- `translate`/`rotate`/`scale`/`origin` are static and state variants may set them; skew and 3D fail.
  `transition-*`/`animate-*` animate paint and transforms only; scaled or rotated editors/scrolls/sliders,
  gradients and `scale-0` jump. `starting:`/`exit:` animate enter and removal; `transition-layout` animates position.
  `ease-spring`, `ease-spring-bouncy` or `ease-[spring(0.4)]` time a transition by a spring; `duration-*` stays the knob.
- Utilities that would draw nothing (filters but `blur-*`, radial masks, text shadows, cursors, outlines) fail naming what to use.
- In `lingxia dev`, saving View code of GPU pages swaps those pages in place, keeping `useLocal`, scroll
  and focus; Logic or config edits rebuild and reopen. Dev builds' runtime errors name `file:line:col`.
- The build reports every problem with `file:line:col`, split into unknown
  utilities, ones the renderer cannot draw, and ones the element ignores.
- `useLocal(value)` state is keyed by call order and component identity; `useLocal(name, value)` by name (survives reorders and hot edits).
- `useRequest`, `useStream(name, action, { onData? })`,
  `useChannel(name, action, { params, onData? })` take `view.actions` references.
  Errors are `{ code, message }`; reloads keep old values (`stale`). `useLeaveGuard(name, { enabled })`.
- Lists: `<List items getKey renderItem />`; hooks inside rows need stable keys.
- Scroll snap: `snap-x`/`snap-y` on a `<scroll>` or `<List>`, `snap-start|center|end` on children; every snap is mandatory.
  Pager: `<scroll bind={page} className="overflow-x-auto snap-x snap-always">`; a flick stops one page on, `bind` is the resting page, two-way.
- `drag="x|y" dragSnapPoints={[0, 120, '50%']}` (≤8, px or % of its own size; `x` mirrors in RTL) lets the
  finger move an element; it springs (its `transition-transform duration-* ease-spring*`) to the nearest point, a flick one on.
  `bind={cell}` is the point's index (setting it springs there), `onDragStart()`, `onDragEnd({ index, velocity })`.
  Needs `aria-label`: screen readers and arrow keys step it; specs read the point as `toHaveValue('1')`.
  A same-axis scroll inside it scrolls only once it is fully open (lowest offset) and returns to its top before it closes.
- No other pointer, swipe, pinch or long-press handlers and no drag-and-drop.
- `<vector paths gradients>`: paths take SVG `transform`, caps, joins, dash offset and
  `fill`/`stroke: 'url(#id)'` naming a `gradients` entry (linear, radial or conic, ≤8).
- Rich text: `<text>Read <span className="font-bold">this</span> or <link onClick={go}>that</link></text>`;
  runs take text paint only; links are accent + underline by default. `select-text` lets users
  long-press to select the nearest word and copy; there a long-press on a link selects, elsewhere it presses.
- Semantics: `role="heading" headingLevel={2}`, `list`/`listitem`, `menu`/`menuitem`, `alert`/`status`
  (announced), `aria-hidden` for decoration. A layout element with a role, or a scroll,
  groups its content for screen readers ("item 2 of 3"); plain wrappers collapse.
  `aria-pressed`, `aria-selected`, `aria-expanded`, `aria-busy` announce states (e.g. `aria-busy={req.status === 'loading'}`).
- `<dialog open onClose closedby aria-label>` is modal, above the page (`backdrop:` styles its scrim;
  `mt-auto mb-0 w-full` makes a sheet). `<view id popover>` + `<button popoverTarget>` is a light-dismiss
  popover anchored to its invoker (`positionArea`, `open`/`onToggle`); `role="menu"` + `<button role="menuitem">` closes on click.
- Swipe row: `<button drag="x" dragSnapPoints={[0, -144, '-100%']} aria-actions="archive-1 delete-1">` over `<button id="archive-1">`s;
  screen readers get those as the row's actions; specs call `locator.performAction('Delete')` (or `increment`/`decrement`/`dismiss`).
- Sheet detents: `<dialog drag="y" dragSnapPoints={[0, '50%', '100%']} bind={detent}>`; resting wholly off screen calls `onClose`, the scrim fades with it.
- Keyboard: Tab and arrows follow reading order (no `tabIndex`); an open `<dialog>` keeps focus inside
  and returns it on close; the focus ring shows only after keyboard input.
- Inputs take `type` (`password`, `email`, `number`, `tel`, `search`, `date`, `time`), `inputMode`, `maxLength`, `autoComplete`, `enterKeyHint` (on a textarea, `send`/`go`/`done`/`search` makes Return submit), `autoCapitalize`, `spellCheck`;
  `type="number"` with `min`/`max`/`step` clamps on blur/submit; `type="date|time"` opens the
  View's own themed picker (popover with a mouse, sheet on touch) and reports ISO strings.
- While an image loads its background colour shows; `placeholder="tiny.jpg"` paints a small stand-in scaled smooth.
- `object-top`, `object-left-bottom` or `object-[25%_75%]` (`objectPosition`) align a covered or contained image; pixel offsets fail.
- `<textarea rows={2} maxRows={6}/>` grows with its lines in that range, then scrolls.
- `<select options={[{value,label,disabled?}]} bind={cell} placeholder aria-label/>` opens the View's own
  picker (popover with a mouse, sheet on touch); arrows change the choice; automation uses `locator.selectOption(valueOrLabel)`.
- `locator.dblclick()` selects the word at an element's center in selectable text and editors (a long press on touch).
- `locator.click()` on content inside a control (a checkbox's label) clicks that control, as a finger would; it fails when the control is disabled.
- `enterKeyHint="next"|"previous"` moves focus to the adjacent enabled editor in reading order
  (the last one submits); `cell.focus()` focuses the editor bound to that `useLocal` cell.
- Buttons may contain text, icons and images; their text names them for
  accessibility. Images need `alt` (`""` for decoration).
- The build fails with `UI_A11Y` when an editor, slider, radio group or plot lacks `aria-label`,
  or a button, link, checkbox or switch has neither text nor a label; a placeholder is not a name.
- Remote `<image>` sources follow HTTP caching: revalidated, and shown stale while offline.
- Text follows the system font size; prefer `min-h-*` over fixed heights. Whitespace
  collapses as in CSS: use `whitespace-pre-line`/`pre-wrap` for authored newlines.
- `UI_COMPAT` means the installed host is older than the bundle: update it.
- `UI_PERF` build warnings do not stop the build: use `<List>` past 50 literal rows, keep opacity and
  backdrop off repeated rows, and ship bundled images at most 2048 px.
