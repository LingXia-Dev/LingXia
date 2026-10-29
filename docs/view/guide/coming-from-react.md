# Coming from React

```tsx data={"query":"lamp"}
import { List, useLocal, useRequest, type View } from '@lingxia/ui';

type Product = { id: string; name: string; price: string };
type Actions = { search(input: { query: string }): Promise<Product[]> };

// React: const [query, setQuery] = useState(''); useEffect(() => { fetch(…) }, [query]);
export function render({ data, actions }: View<Actions, { query: string }>) {
  const query = useLocal('query', data.query);
  const results = useRequest('results', actions.search, { query: query.value });
  return (
    <page>
      <column className="p-4 gap-2">
        <search bind={query} aria-label="Search products" placeholder="Search" />
        {results.pending && <text className="text-xs text-muted">Searching…</text>}
      </column>
      <List
        items={results.value ?? []}
        getKey={product => product.id}
        renderItem={product => (
          <row className="justify-between px-4 py-3">
            <text className="text-sm">{product.name}</text>
            <text className="text-sm tabular-nums">{product.price}</text>
          </row>
        )}
      />
    </page>
  );
}
```

The syntax is JSX and the model is the same — render from state, re-render on change — but there is
no React and no DOM. This page lists what changes.

## Elements

| HTML / React | View |
| --- | --- |
| `div` | `<view>` / `<column>`, `<row>`, `<grid>`, `<stack>` |
| `p`, `h1`–`h6`, `span` with text | `<text>` (headings add `role="heading" headingLevel`) |
| inline `span`, `strong`, `em`, `code` | `<span>` inside `<text>` |
| `a` | `<link onPress>` inside `<text>`, or a `<button>` |
| `img` | `<image>` |
| `svg`, icon components | `<vector paths>` |
| `ul`/`ol`/`li` | `<column role="list">` / `role="listitem"` |
| `input`, `textarea` | `<input>`, `<textarea>`, `<search>` |
| `input type="checkbox"`, `select`, `input type="range"` | `<checkbox>`/`<switch>`, `<radio>`, `<slider>` |
| `form` | none: state + an action ([Inputs and forms](inputs-and-forms.md)) |
| `canvas`, `video`, `iframe` | none; `<plot>` for charts |
| scrolling `div` | `<scroll>`; long lists `<List>` |

Unknown tags fail the build (`UI_TAG`). Text must be inside `<text>`; a bare string becomes one.

## Hooks

| React | View |
| --- | --- |
| `useState` | `useLocal(name, initial)`, keyed by name, JSON values only |
| `useEffect` + `fetch` | `useRequest(name, action, params)` |
| `useEffect` subscriptions, WebSockets | `useChannel`, `useStream` |
| `useContext` | props, or `useView()` for the page data and environment |
| `useRef` to a DOM node | none: there are no nodes to hold |
| `useMemo`, `useCallback`, `memo` | not needed; compute inline |
| `useReducer` | a `useLocal` cell and plain functions |
| `beforeunload` / router blockers | `useLeaveGuard` |

There are no effects and no lifecycle callbacks in the View. Side effects happen in event handlers and
in Logic. `render` is synchronous: no Suspense, `lazy`, async components, portals or error boundaries.
A render that throws keeps the previous frame on screen ([Diagnostics](diagnostics.md)).

## Components and JSX

- Fragments work as in React, except as the page root (`UI_FRAGMENT`) or a `<list>` row.
- `key` must be a string.
- `{count && <x/>}` renders `0` when `count` is `0`, as in React.
- Children are passed through as given; there is no `React.Children`, only `childrenToArray`.
- React component libraries do not work: they render DOM. Plain TypeScript libraries do, if they
  avoid DOM, timers and `Intl`.

## Events

- Only controls have handlers: `onClick` on `<button>`, `onPress` on `<link>`, `onChange`/`onInput` on
  controls. There are no `onClick` handlers on containers; wrap the content in a `<button>`.
- Handlers receive values (`onInput(text)`, `onChange(checked)`), not event objects. There is no bubbling,
  `preventDefault` or `stopPropagation`.
- No pointer, touch, keyboard, focus or scroll events; use `onReachStart`/`onReachEnd`, anchors and state.

## Styling

- Tailwind utilities and the `style` prop only. CSS files, CSS modules, CSS-in-JS and `<style>` do not
  exist; the one stylesheet a page may import holds theme tokens.
- Write complete class names. Helpers such as `clsx` work when every class is a complete literal, but the
  build can check only classes it can trace through literals, conditionals and `const`s. Conflicts resolve
  as tailwind-merge does (the later class wins per property and variant), so a `cn` that only joins is enough.
- There is no block or inline flow: every box is flexbox or grid, and `display: inline` is not supported.
  Flex items do not shrink by default ([Layout](layout.md)).
- There is no stylesheet cascade. Text styles inherit through containers; nothing else inherits.
- `data-*` variants match `data-*` props on the element itself; `has-*` covers checked, disabled and
  focused descendants; `group-active:` follows the enclosing pressed control. `aria-*`, other `group-*`,
  `peer-*` and container queries are refused: compute the condition in TypeScript and choose the class.
- A shadcn-style theme ports by aliasing its colour names to roles in `@theme` (`--color-card:
  var(--color-surface)`), its classes become `@utility` rules ([Styling](styling.md#themes-and-tokens)).
  Two names are roles already and cannot be rebound: write its soft `bg-accent` as `bg-accent-soft` and its
  grey `bg-muted` as `bg-surface-sunken`.
- There is no preflight: a `<button>` paints the accent with 12 px padding until its classes say
  otherwise. A web button styled from Tailwind's reset needs `bg-transparent p-0`.
- Transitions and animations are utilities ([Motion](motion.md)), including `starting:`/`exit:` and
  `transition-layout`; size transitions and 3D transforms are not supported.

## The DOM and the browser

The View has no `window`, `document`, `localStorage`, `fetch`, `XMLHttpRequest`, `setTimeout`,
`setInterval`, `requestAnimationFrame`, `matchMedia` or `Intl`, and no measurement APIs such as
`getBoundingClientRect`. Replace them with:

| Browser | View |
| --- | --- |
| `fetch`, storage, timers | Logic, reached through actions, streams and channels |
| `matchMedia('(prefers-color-scheme: dark)')` | `env.theme`, `dark:` |
| `window.innerWidth`, media queries | `env.viewport`, `sm:`/`md:`/… |
| `navigator.language`, `Intl` | `env.displayLanguage`; format in Logic |
| `element.scrollIntoView()` | a `<List>` `anchor` |
| `user-select: text` by default | text is not selectable until `select-text` |
| a selection across elements | selection stays within one `<text>` |
