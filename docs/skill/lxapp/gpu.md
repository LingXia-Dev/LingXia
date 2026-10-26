# GPU Views (experimental, Android)

A page renders natively when its `index.json` has `"renderer": "gpu"`; its View
is `index.tsx` (Logic stays in `index.ts`). Add `@lingxia/ui` as a dependency.
There is no DOM. The host owns navbar and tabbar.

```tsx
import { useLocal, useRequest, type View } from '@lingxia/ui';

type Actions = { load(q: { id: string }): Promise<{ title: string }> };

export function render({ data, actions }: View<Actions, { id: string }>) {
  const draft = useLocal('draft', '');
  const item = useRequest('item', actions.load, { id: data.id });
  return <page className="p-4 gap-3">
    <text className="text-lg font-semibold truncate">{item.value?.title ?? 'Loading…'}</text>
    <input bind={draft} accessibilityLabel="Note" className="px-3 py-2 rounded-lg border" />
    <button disabled={!draft.value} onClick={() => draft.set('')} className="active:opacity-80">Clear</button>
  </page>;
}
```

- `<page>` is the root: it absorbs safe-area insets and the keyboard. `view` is a column.
  `fixed` insets and `vh`/`dvh` measure the page's safe box (inside system bars, above the keyboard);
  `hidden` removes a node entirely.
- Layout is flex or CSS Grid (`grid grid-cols-[repeat(auto-fit,minmax(8rem,1fr))] col-span-2`).
- Style with complete Tailwind classes or CSS-named `style` (`backgroundColor`,
  `marginTop`, `gridTemplateColumns`, `lineClamp`, `objectFit`). Unitless
  `lineHeight` is a multiplier. Each element accepts only styles it renders.
- RTL: use `ps`/`ms`/`start-`/`text-start` etc.; they mirror with `env.direction` (from the
  display language) or a `dir` prop. Mirror icons with `rtl:-scale-x-100`.
- Unstyled pages, text, buttons and editors take light or dark default colors from `env.theme`.
- Supported variants: `dark:`, `rtl:`/`ltr:`, `first:`/`last:`/`odd:`/`even:`/`only:`/`empty:`, `sm:`–`2xl:`, `focus:` (editors), `active:`
  (controls), `hover:` (fine pointers), `disabled:`, `placeholder:`,
  `space-*`/`divide-*`. `*:`, `**:`, `in-*:`, `group-*:`, `peer-*:` fail naming the TS-side form.
- `translate`/`rotate`/`scale`/`origin` are static and state variants may set them; skew and 3D fail.
  Transitions and `animate-*` need `// @ui-experimental motion` in the page source.
- Utilities that would draw nothing (filters, masks, text shadows, cursors, outlines) fail naming what to use.
- The build reports every problem with `file:line:col`, split into unknown
  utilities, ones the renderer cannot draw, and ones the element ignores.
- `useLocal(name, value)` state is keyed by name and component identity.
- `useRequest`, `useStream(name, action, { onData? })`,
  `useChannel(name, action, { params, onData? })` take `view.actions` references.
  Errors are `{ code, message }`; reloads keep old values (`stale`). `useLeaveGuard(name, { enabled })`.
- Lists: `<List items getKey renderItem />`; hooks inside rows need stable keys.
- `<vector paths gradients>`: paths take SVG `transform`, caps, joins, dash offset and
  `fill`/`stroke: 'url(#id)'` naming a `gradients` entry (linear, radial or conic, ≤8).
- Rich text: `<text>Read <span className="font-bold">this</span> or <link onPress={go}>that</link></text>`;
  runs take text paint only; links are accent + underline by default. `select-text` lets users
  long-press to select the nearest word and copy; there a long-press on a link selects, elsewhere it presses.
- Semantics: `role="heading" headingLevel={2}`, `list`/`listitem`, `dialog`, `alert`/`status`
  (announced), `accessibilityHidden` for decoration. A layout element with a role, or a scroll,
  groups its content for screen readers ("item 2 of 3"); plain wrappers collapse.
  `accessibilityState={{ pressed, selected, expanded, busy }}` announces states (e.g. `busy: req.status === 'loading'`).
- Inputs take `maxLength`, `autoComplete` (autofill), `enterKeyHint`, `autoCapitalize`, `spellCheck`;
  `keyboard="number"` with `min`/`max`/`step` clamps on blur/submit; `keyboard="date|time"` opens the
  platform picker and reports ISO strings.
- `enterKeyHint="next"|"previous"` moves focus to the adjacent enabled editor in reading order
  (the last one submits); `cell.focus()` focuses the editor bound to that `useLocal` cell.
- Buttons may contain text, icons and images; their text names them for
  accessibility. Images need `alt` (`""` for decoration).
- The build fails with `UI_A11Y` when an editor, slider, radio group or plot lacks `accessibilityLabel`,
  or a button, link, checkbox or switch has neither text nor a label; a placeholder is not a name.
- Remote `<image>` sources follow HTTP caching: revalidated, and shown stale while offline.
- Text follows the system font size; prefer `min-h-*` over fixed heights. Whitespace
  collapses as in CSS: use `whitespace-pre-line`/`pre-wrap` for authored newlines.
- `UI_COMPAT` means the installed host is older than the bundle: update it.
