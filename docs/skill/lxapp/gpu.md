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
  `fixed` positions against the page; `hidden` removes a node entirely.
- Style with complete Tailwind classes or CSS-named `style` (`backgroundColor`,
  `marginTop`, `gridTemplateColumns`, `lineClamp`, `objectFit`). Unitless
  `lineHeight` is a multiplier. Each element accepts only styles it renders.
- Supported variants: `dark:`, breakpoints, `focus:` (editors), `active:`
  (controls), `hover:` (fine pointers), `disabled:`, `placeholder:`,
  `space-*`/`divide-*`. Transitions and transforms are rejected.
- The build reports every problem with `file:line:col`, split into unknown
  utilities, ones the renderer cannot draw, and ones the element ignores.
- `useLocal(name, value)` state is keyed by name and component identity.
- `useRequest`, `useStream(name, action, { onData? })`,
  `useChannel(name, action, { params, onData? })` take `view.actions` references.
  Errors are `{ code, message }`; reloads keep old values (`stale`). `useLeaveGuard(name, { enabled })`.
- Lists: `<List items getKey renderItem />`; hooks inside rows need stable keys.
- `<vector paths gradients>`: paths take SVG `transform`, caps, joins, dash offset and
  `fill`/`stroke: 'url(#id)'` naming a `gradients` entry (linear or radial, ≤8).
- Buttons may contain text, icons and images; their text names them for
  accessibility. Images need `alt` (`""` for decoration).
- Text follows the system font size; prefer `min-h-*` over fixed heights.
- `UI_COMPAT` means the installed host is older than the bundle: update it.
