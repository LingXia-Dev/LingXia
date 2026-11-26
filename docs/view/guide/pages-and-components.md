# Pages and components

```tsx data={"items":[{"id":"a","title":"Milk","done":false},{"id":"b","title":"Bread","done":true}]}
import { useLocal, type Child } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  return (
    <page className="p-4 gap-3">
      <Card title="Groceries">
        {data.items.map(item => (
          <Row key={item.id} title={item.title} done={item.done} onToggle={() => actions.toggle({ id: item.id })} />
        ))}
      </Card>
    </page>
  );
}

function Card({ title, children }: { title: string; children?: Child }) {
  return (
    <view className="rounded-xl border border-line p-4 gap-2">
      <text className="text-base font-semibold">{title}</text>
      {children}
    </view>
  );
}

function Row({ title, done, onToggle }: { title: string; done: boolean; onToggle: () => void }) {
  const expanded = useLocal('expanded', false);
  return (
    <view className="gap-1">
      <view className="flex-row items-center gap-2">
        <checkbox value={done} onChange={onToggle} aria-label={title} />
        <button onClick={() => expanded.set(!expanded.value)} className="grow p-0 bg-transparent">
          <text className={done ? 'text-sm line-through text-faint' : 'text-sm'}>{title}</text>
        </button>
      </view>
      {expanded.value && <text className="text-xs text-muted">Tap the box to mark it done.</text>}
    </view>
  );
}
```

Its Logic, `index.ts`:

```ts
import { definePage } from '@lingxia/logic';

type Item = { id: string; title: string; done: boolean };

export default definePage({
  data: { items: [] as Item[] },
  actions: page => ({
    toggle({ id }: { id: string }) {
      page.update(d => { const item = d.items.find(item => item.id === id); if (item) item.done = !item.done; });
    },
  }),
});
```

## A page

A page renders natively when its View exports a named `render`; every page of the lxapp must, and
`lxapp.json` names no renderer:

```json
{ "appId": "groceries", "version": "1.0.0", "pages": [{ "name": "list", "path": "pages/list/index" }] }
```

A page is a directory under `pages/`. Its optional `index.json` sets its chrome:

```json
{ "navigationBar": { "title": "Groceries" } }
```

`index.tsx` is the View and exports `render`. `index.ts` is the page's Logic (`export default definePage({ data, setup })`)
and is optional. Add `@lingxia/view` to the lxapp's dependencies; `lingxia build` compiles every page.

`render(view)` runs whenever `view.data`, `view.env` or local state changes. `view.data` is the Logic
page's data, deeply read-only. `view.actions` holds Logic's `actions`: each returns a `Promise`, and an
`async *action()` generator becomes a streaming action (see [State and data](state-and-data.md)).

## The root

Return `<page>` as the root. It absorbs the safe-area insets and the keyboard into its padding, and
a `<scroll>` or `<List>` directly inside it fills the remaining height. `<page>` is valid only as the
root. The page must return exactly one element (`UI_ROOT`), so a fragment cannot be the root (`UI_FRAGMENT`).

```tsx error UI_FRAGMENT
export function render() {
  return <><text>One</text><text>Two</text></>;
}
```

Wrap siblings in a `<view>` or `<page>` instead. `<view>` is a flex column; `flex-row` (or `flex`) makes it a row.

Anywhere else, a fragment groups children without a box: `<>…</>` or a component that returns one lays
its children out in its place, and each keeps its own identity and local state while siblings come and
go. `<Fragment key>` (from `@lingxia/view`) names a group in a keyed map, so it moves as one:

```tsx
import { Fragment, useLocal } from '@lingxia/view';

function Term({ word, meaning }: { word: string; meaning: string }) {
  return <><text className="font-semibold">{word}</text><text className="text-muted">{meaning}</text></>;
}

export function render() {
  const words = useLocal('words', [['GPU', 'draws the page'], ['View', 'describes it']]);
  return (
    <page className="p-4 gap-1">
      {words.value.map(([word, meaning]) => <Fragment key={word}><Term word={word} meaning={meaning} /></Fragment>)}
    </page>
  );
}
```

A `<List>` row and a component inside `<text>` still return one element.

## Components

A component is a function from props to one element, `null`, `undefined` or a boolean (nothing).
Components receive props only. Read the page View inside one with `useView()`; pass typed data
down as props when the component needs its type. A component that needs only the environment reads
`useEnv()`: it then re-runs only when an environment field it read changes, not on every
`view.data` change as a `useView()` reader does.

```tsx
import { useEnv, type Child } from '@lingxia/view';

function Section({ title, children }: { title: string; children?: Child }) {
  const env = useEnv();
  return (
    <view className={env.appearance === 'dark' ? 'gap-2 p-4 bg-text' : 'gap-2 p-4 bg-surface-sunken'}>
      <text className="text-sm font-semibold">{title}</text>
      {children}
    </view>
  );
}

export function render() {
  return <page><Section title="Account"><text>Signed in</text></Section></page>;
}
```

`children` is exactly what the caller passed: `undefined`, one child, or an array. Use
`childrenToArray(children)` to get a flat array without empty values.

## Taking a child as the trigger

`elementType(child)` says what a child is: its tag name, its component (compare by identity), or
`undefined` for text and empty values. `cloneElement(child, props)` returns the child with `props`
merged over its own, in the child's place, so its local state carries over. `className` is joined;
`key` and `children` cannot change. Adding a handler the child already has throws `UI_PROP` unless
it is named in `{ override: [...] }`; the child's own handler is not readable, so wrap the child
when both must run.

```tsx
import { useLocal, childrenToArray, cloneElement, elementType, type Child } from '@lingxia/view';

function Disclosure({ children }: { children?: Child }) {
  const open = useLocal(false);
  const [trigger, ...body] = childrenToArray(children);
  if (elementType(trigger) !== 'button') throw Error('Disclosure takes a <button> first');
  return (
    <view className="gap-2">
      {cloneElement(trigger, { 'aria-expanded': open.value, onClick: () => open.set(!open.value) })}
      {open.value && body}
    </view>
  );
}

export function render() {
  return (
    <page>
      <Disclosure>
        <button className="p-2"><text>Shipping</text></button>
        <text>Ships in two days.</text>
      </Disclosure>
    </page>
  );
}
```

## Children and conditionals

- `null`, `undefined`, booleans and `''` render nothing, so `{flag && <x/>}` is safe.
- A number renders as text: `{count && <x/>}` shows `0` when `count` is `0`. Write `{count > 0 && <x/>}`.
- A string or number among element children becomes a `<text>`.
- Arrays flatten. Siblings produced by `map` need keys when they can reorder.

## Keys

`key` is a string, unique among its siblings (`UI_KEY` otherwise). It gives a child an identity that
survives reordering, so its local state and resources move with it. Unkeyed children are identified
by position.

```tsx error UI_KEY
export function render() {
  return (
    <page>
      {['a', 'a'].map(id => <text key={id}>{id}</text>)}
    </page>
  );
}
```

## Local state belongs to a component instance

`useLocal(name, initial)` is keyed by `name` and the component's position (or key path) in the tree,
not by call order; `useLocal(initial)` is keyed by call order within the component. A position counts the authored children, so `{cond && <Banner />}` appearing or
disappearing leaves the state of the siblings after it alone. Two `useLocal` calls with the same name in one component fail (`UI_LOCAL`). A cell
that a render does not declare is dropped when that render commits, so a conditionally declared
cell loses its value while the condition is false.

A component's identity is its module and declaration name (`pages/lab/index#Inputs`), fixed at build
time, so minified release builds and hot swaps agree. Declare components at the top level of a module;
keying by `Component.name` fails the build (`UI_KEY`).

## Context

`createContext(default)` hands a value to a whole subtree, for parts of one compound component. Declare
it as a module-level `const` (`UI_CONTEXT` otherwise). `<X.Provider value={…}>` wraps exactly one
element; `useContext(X)` returns the nearest provider's value, or the default. Values are plain data,
functions or handles such as a `Local`; to change one, provide a `Local` and `set` it from a handler.
A changed value re-runs only the components that read it, including `List` rows.

Every element takes an `id`, unique on the page (`UI_ID`). Inside a component, build ids with
`useId()`: it is unique per component instance and list row, and stable across renders and hot swaps,
so a compound control can name its parts (`` `${id}-panel` ``).

```tsx
import { createContext, useContext, useLocal, type Child, type Local } from '@lingxia/view';

const Selected = createContext<Local<string> | null>(null);

function Choice({ value }: { value: string }) {
  const selected = useContext(Selected);
  const on = selected?.value === value;
  return <button onClick={() => selected?.set(value)} className={on ? 'bg-accent' : ''}>{value}</button>;
}

function Choices({ children }: { children?: Child }) {
  const selected = useLocal('selected', 'a');
  return <Selected.Provider value={selected}><view className="flex-row gap-2">{children}</view></Selected.Provider>;
}

export function render() {
  return <page><Choices><Choice value="a" /><Choice value="b" /></Choices></page>;
}
```

## data-testid

Every element takes `data-testid`, the name automation (`lxdev`, the test runner) uses to find it. It has
no visual or accessibility effect.

Locators read a native page like a DOM page:

- An element's text is its rendered text, or its descendants' text in order when it has none, as
  `textContent`/`toContainText` read it in a WebView. Editor values stay values.
- `[placeholder="…"]` finds an unlabelled editor by its placeholder.
- `data-*` props are attributes: `[data-mode="dark"]` and `[data-selected]` select by them,
  and `getAttribute`/`toHaveAttribute` read them (`true` reads as `""`). They never reach accessibility
  and draw nothing; at most 16 per element and 64 KiB per page (`dataAttributeBytes`).
- Authored ARIA states are attributes too: `aria-pressed`, `-selected`, `-expanded`, `-busy`,
  `-invalid`, `-required` and `-checked` read as `"true"` or `"false"`, `aria-activedescendant` as
  the id, and `[aria-pressed="true"]` selects by them.
- `page.action(name, args)` calls a Logic action exactly as `view.actions[name](args)` does and
  resolves with its result once the frame it produced is committed; `page.data()` reads `view.data`.
- A page that left the stack keeps its last frame for reading: queries, the tree, text and
  attributes answer from it. Clicks, fills, scrolls and gestures fail with `E_PAGE_NOT_READY`
  (parked) until it is shown again.
