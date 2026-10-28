# Diagnostics

```tsx
import { useRequest, type ResourceError, type View } from '@lingxia/ui';

type Actions = { orders(): Promise<{ id: string; total: string }[]> };

function ErrorBanner({ error, onRetry }: { error: ResourceError | null; onRetry: () => void }) {
  if (!error) return null;
  return (
    <row role="alert" className="items-center gap-3 p-3 rounded-lg bg-danger-soft">
      <text className="grow text-sm text-danger">{error.message}</text>
      <button onClick={onRetry} className="rounded-lg bg-danger text-on-accent px-3 py-1">Retry</button>
    </row>
  );
}

export function render({ actions }: View<Actions>) {
  const orders = useRequest('orders', actions.orders);
  return (
    <page className="p-4 gap-3">
      <ErrorBanner error={orders.error} onRetry={() => orders.reload()} />
      {(orders.value ?? []).map(order => (
        <row key={order.id} className="justify-between">
          <text className="text-sm">{order.id}</text>
          <text className="text-sm tabular-nums">{order.total}</text>
        </row>
      ))}
    </page>
  );
}
```

Problems surface in three places: the build, the View runtime, and the host. Each message starts with
a code (`UI_…: detail`); the meaning of every code is in [errors](../reference/errors.md).

## Build errors

`lingxia build` checks every GPU page before it writes anything. It reports all problems of a page at
once, each with its source position, and writes no module for a page that has any:

```text
pages/profile/index.tsx: 5 authoring problems
pages/profile/index.tsx:4:7: UI_TAG: unsupported tag <div>
pages/profile/index.tsx:5:39: UI_PROP: onClick is not a property of <text>
pages/profile/index.tsx:4:22: UI_CLASS_UNKNOWN: flex-center is not a Tailwind utility
pages/profile/index.tsx:5:23: UI_CLASS_ELEMENT: gap-2 sets gap, which <text> does not support
pages/profile/index.tsx:6:25: UI_CLASS_ELEMENT: active:p-2: active styles change paint only, not paddingTop, paddingRight, paddingBottom, paddingLeft
```

| Code | Fix |
| --- | --- |
| `UI_TAG` | use an element of the language ([elements](../reference/elements.md)) |
| `UI_PROP` | the element does not take that prop; spreads are checked too |
| `UI_STYLE` | an inline `style` key the element does not render |
| `UI_CLASS_UNKNOWN` | not a Tailwind utility; the message names the nearest supported one when it is close |
| `UI_CLASS_UNSUPPORTED` | real Tailwind the renderer cannot draw; the message says what and often what to use instead |
| `UI_CLASS_ELEMENT` | a utility on an element that ignores it, a state variant the element never enters, or a state style that changes layout |
| `UI_DOM` | `window`, `document`, `localStorage` or `HTMLElement` in View code |
| `UI_FRAGMENT` | the page returns a fragment; put it inside `<page>` |
| `UI_CSS` | the stylesheet holds more than tokens, or a page imports two |
| `UI_ANIMATION` | an inline `style.animation` or invalid `@keyframes` ([Motion](motion.md)) |

A class problem points at the string literal that holds the class, which may be in another module
(a shared `const` or class map). Line and column are 1-based.

`UI_PERF` lines are warnings: the page still builds. They flag more than 50 rows from a literal set
outside `<List>`, an `opacity-*` or `backdrop-*` layer repeated in every row, and a bundled image
longer than 2048 px or larger than 8 MiB.

## Runtime errors

A render that throws fails its frame. The error keeps its code and gains the JSX path it was built from,
innermost first:

```text
UI_KEY: duplicate sibling
  at <row> (pages/profile/index.tsx:3:5)
  at Tags (pages/profile/index.tsx:10:16)
  at <page> (pages/profile/index.tsx:10:10)
```

`<tag>` lines are elements, bare names are components. The page keeps showing its last good frame;
the host offers a Retry that renders again in the same session. Read these messages with
`lxdev logs --grep UI_`. At debug level every executed tap also logs a
`UI_INPUT: click page=… node=… commit=… current=… reresolved=…` line, and a tap whose handler is gone
warns as a dropped activation.

Common runtime codes: `UI_KEY` (duplicate or non-string keys), `UI_LOCAL` (a duplicate `useLocal` name, or
`set` on an unmounted cell), `UI_RENDER` (state changed or an action called during render), `UI_CLASS`
(a class name built at runtime), `UI_LIST` (keys, anchors, row hooks), `UI_INPUT`/`UI_CHECK`/`UI_RADIO`/
`UI_SLIDER` (control values and bindings).

## Resource errors are values

Failed requests, streams, channels and leave guards never throw into render. They set `error` to
`{ code, message }` on the resource, as in the first example. Codes from Logic (your own, or the
platform's) pass through unchanged.

## Host errors

The host validates every frame again before laying it out. A frame it refuses (`UI_STYLE` for a sticky
box outside a scroll, `UI_LIMIT`, `UI_GRID`, …) is dropped and the previous frame stays on screen, with
the message in the log. View code that runs too long is stopped with `UI_TIMEOUT`; the page then needs a
reload, which resets its local state.

## `UI_COMPAT`

```text
UI_COMPAT: this UI bundle requires a compatible native host (ABI <abi>, field <n>, candidate-transactions, bounded-jobs)
```

The page was compiled for a newer host than the one installed. Nothing of the page runs. Update the host
app (or the SDK it embeds); rebuilding the page does not help. See
[Limits and compatibility](limits-and-compatibility.md).
