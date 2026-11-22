# Coming from React

```tsx data={"query":"lamp"}
import { List, useLocal, useRequest } from '@lingxia/view';
import type { View } from './$types';

// React: const [query, setQuery] = useState(''); useEffect(() => { fetch(…) }, [query]);
export function render({ data, actions }: View) {
  const query = useLocal('query', data.query);
  const results = useRequest('results', actions.search, { query: query.value });
  return (
    <page>
      <view className="p-4 gap-2">
        <input type="search" bind={query} aria-label="Search products" placeholder="Search" />
        {results.pending && <text className="text-xs text-muted">Searching…</text>}
      </view>
      <List
        items={results.value ?? []}
        getKey={product => product.id}
        renderItem={product => (
          <view className="flex-row justify-between px-4 py-3">
            <text className="text-sm">{product.name}</text>
            <text className="text-sm tabular-nums">{product.price}</text>
          </view>
        )}
      />
    </page>
  );
}
```

The network call lives in its Logic, `index.ts`, where `fetch` and `lx` are:

```ts
import { definePage } from '@lingxia/logic';

type Product = { id: string; name: string; price: string };
const catalog: Product[] = [{ id: 'p1', name: 'Desk lamp', price: '29.00' }];

export default definePage({
  data: { query: '' },
  actions: {
    async search({ query }: { query: string }) {
      return catalog.filter(product => product.name.toLowerCase().includes(query.toLowerCase()));
    },
  },
});
```

The syntax is JSX and the model is the same — render from state, re-render on change — but there is
no React and no DOM. This page lists what changes.

## Elements

| HTML / React | View |
| --- | --- |
| `div` | `<view>` (`flex-row`, `grid` as in Tailwind) |
| `p`, `h1`–`h6`, `span` with text | `<text>` (headings add `role="heading" headingLevel`) |
| inline `span`, `strong`, `em`, `code` | `<span>` inside `<text>` |
| `a` | `<link onClick>` inside `<text>`, or a `<button>` |
| `img` | `<image>` |
| `svg`, icon components | `<vector paths>` |
| `ul`/`ol`/`li` | `<view role="list">` / `role="listitem"` |
| `input`, `textarea` | `<input>`, `<textarea>` |
| `input type="checkbox"`, radio buttons, `select`, `input type="range"` | `<checkbox>`/`<switch>`, `<radio>`, `<select>`, `<slider>` |
| `form` | none: state + an action ([Inputs and forms](inputs-and-forms.md)) |
| `video` | `<video>` (`useVideo(name)` shares a player with a full-screen `<dialog>`) |
| `iframe` | `<webview>` (`useWebView(name)` shares a live document with a `<dialog>` or sheet) |
| `canvas` | none; `<plot>` for charts, `<vector>` for shapes |
| scrolling `div` | `<scroll>`; long lists `<List>` |

Unknown tags fail the build (`UI_TAG`). Text must be inside `<text>`; a bare string becomes one.

## Hooks

| React | View |
| --- | --- |
| `useState` | `useLocal(initial)`, keyed by call order, or `useLocal(name, initial)`, keyed by name; JSON values only |
| `useEffect` + `fetch` | `useRequest(name, action, params)` |
| `useEffect` subscriptions, WebSockets | `useChannel`, `useStream` |
| `useContext` | props, or `useView()` for the page data and environment |
| `useRef` to a DOM node | none: there are no nodes to hold |
| `useMemo`, `useCallback`, `memo` | not needed: a component re-runs only when its props, the cells it reads or the environment change, and closures keep their identity while what they capture is unchanged |
| `useReducer` | a `useLocal` cell and plain functions |
| `beforeunload` / router blockers | `useLeaveGuard` |

There are no effects and no lifecycle callbacks in the View. Side effects happen in event handlers and
in Logic. `render` is synchronous: no Suspense, `lazy`, async components, portals or error boundaries.
A render that throws keeps the previous frame on screen ([Diagnostics](diagnostics.md)).

## Components and JSX

- Fragments work as in React, except as the page root (`UI_FRAGMENT`) or a `<List>` row.
- `key` must be a string.
- `{count && <x/>}` renders `0` when `count` is `0`, as in React.
- Children are passed through as given; there is no `React.Children`, only `childrenToArray`,
  `elementType` and `cloneElement` (props only; replacing a handler needs `override`).
- React component libraries do not work: they render DOM. Plain TypeScript libraries do, if they
  avoid DOM, timers and `Intl`.

## Events

- Only controls have handlers: `onClick` on `<button>` and `<link>`, `onChange`/`onInput` on
  controls. There are no `onClick` handlers on containers; wrap the content in a `<button>`.
- Handlers receive values (`onInput(text)`, `onChange(checked)`), not event objects. There is no bubbling,
  `preventDefault` or `stopPropagation`.
- No pointer, touch, keyboard, focus or scroll events; use `onReachStart`/`onReachEnd`/`onScrollEdge`, anchors and state.

## Styling

- Tailwind utilities and the `style` prop only. CSS files, CSS modules, CSS-in-JS and `<style>` do not
  exist; the one stylesheet a page may import holds theme tokens.
- Write complete class names, and join them with `cx` from `@lingxia/view` instead of `clsx`: the build
  checks the classes in its arguments, and cannot see into another helper. Conflicts resolve as
  tailwind-merge does (the later class wins per property and variant), so no `cn` is needed.
- There is no block or inline flow: every box is flexbox or grid, and `display: inline` is not supported.
  Flex items shrink to min-content as in CSS ([Layout](layout.md)).
- There is no stylesheet cascade. Text styles inherit through containers; nothing else inherits.
- `data-*` variants match `data-*` props on the element itself; `has-*` covers checked, disabled and
  focused descendants; `group-active:` follows the enclosing pressed control. `aria-*`, other `group-*`,
  `peer-*` and container queries are refused: compute the condition in TypeScript and choose the class.
- A shadcn-style theme ports by aliasing its colour names to roles in `@theme` (`--color-card:
  var(--color-surface)`), its classes become `@utility` rules ([Styling](styling.md#themes-and-tokens)).
  Two names are roles already and cannot be rebound: write its soft `bg-accent` as `bg-accent-soft` and its
  grey `bg-muted` as `bg-surface-sunken`.
- A `<button>` is unstyled, as after Tailwind's preflight: give it its fill, padding and radius.
- Transitions and animations are utilities ([Motion](motion.md)), including `starting:`/`exit:` and
  `transition-layout` and `transition-size`; 3D transforms are not supported.

## The DOM and the browser

The View has no `window`, `document`, `localStorage`, `fetch`, `XMLHttpRequest`, `setTimeout`,
`setInterval`, `requestAnimationFrame`, `matchMedia` or `Intl`, and no measurement APIs such as
`getBoundingClientRect`. Replace them with:

| Browser | View |
| --- | --- |
| `fetch`, storage, timers | Logic, reached through actions, streams and channels |
| `matchMedia('(prefers-color-scheme: dark)')` | `env.theme`, `dark:` |
| `window.innerWidth`, media queries | `env.viewport`, `sm:`/`md:`/… |
| `navigator.language`, `Intl` | `env.language`; format in Logic |
| an element's size (`getBoundingClientRect`, `ResizeObserver`) | `useBox(name)` and `box={box}`, or `onLayout` |
| `element.scrollIntoView()` | `reveal(id)` for a child of a `<scroll>`; a `<List>` `anchor` for a row |
| `user-select: text` by default | text is not selectable until `select-text` |
| a selection across elements | selection stays within one `<text>` |
