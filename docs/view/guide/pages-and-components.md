# Pages and components

```tsx data={"items":[{"id":"a","title":"Milk","done":false},{"id":"b","title":"Bread","done":true}]}
import { useLocal, type Child, type View } from '@lingxia/ui';

type Item = { id: string; title: string; done: boolean };
type Actions = { toggle(input: { id: string }): Promise<void> };

export function render({ data, actions }: View<Actions, { items: Item[] }>) {
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
    <column className="rounded-xl border border-line p-4 gap-2">
      <text className="text-base font-semibold">{title}</text>
      {children}
    </column>
  );
}

function Row({ title, done, onToggle }: { title: string; done: boolean; onToggle: () => void }) {
  const expanded = useLocal('expanded', false);
  return (
    <column className="gap-1">
      <row className="items-center gap-2">
        <checkbox value={done} onChange={onToggle} accessibilityLabel={title} />
        <button onClick={() => expanded.set(!expanded.value)} className="grow p-0 bg-transparent">
          <text className={done ? 'text-sm line-through text-faint' : 'text-sm'}>{title}</text>
        </button>
      </row>
      {expanded.value && <text className="text-xs text-muted">Tap the box to mark it done.</text>}
    </column>
  );
}
```

## A page

A page is a directory under `pages/`. Its `index.json` opts it into native rendering:

```json
{ "renderer": "gpu", "navigationBar": { "title": { "default": "Groceries" } } }
```

`index.tsx` is the View and exports `render`. `index.ts` is the page's Logic (`Page({ data, …methods })`)
and is optional. Add `@lingxia/ui` to the lxapp's dependencies; `lingxia build` compiles every page
whose config says `"renderer": "gpu"`, and only those.

`render(view)` runs whenever `view.data`, `view.env` or local state changes. `view.data` is the Logic
page's data, deeply read-only. `view.actions` holds Logic's methods: each returns a `Promise`, and an
`async *method()` generator becomes a streaming action (see [State and data](state-and-data.md)).

## The root

Return `<page>` as the root. It absorbs the safe-area insets and the keyboard into its padding, and
a `<scroll>` or `<list>` directly inside it fills the remaining height. `<page>` is valid only as the
root. The page must return exactly one element (`UI_ROOT`); fragments are refused (`UI_FRAGMENT`).

```tsx error UI_FRAGMENT
export function render() {
  return <><text>One</text><text>Two</text></>;
}
```

Wrap siblings in a `<column>`, `<row>` or `<page>` instead. `<view>` is a `<column>`.

## Components

A component is a function from props to one element, `null`, `undefined` or a boolean (nothing).
Components receive props only. Read the page View inside one with `useView()`; pass typed data
down as props when the component needs its type.

```tsx
import { useView, type Child } from '@lingxia/ui';

function Section({ title, children }: { title: string; children?: Child }) {
  const { env } = useView();
  return (
    <column className={env.theme === 'dark' ? 'gap-2 p-4 bg-text' : 'gap-2 p-4 bg-surface-sunken'}>
      <text className="text-sm font-semibold">{title}</text>
      {children}
    </column>
  );
}

export function render() {
  return <page><Section title="Account"><text>Signed in</text></Section></page>;
}
```

`children` is exactly what the caller passed: `undefined`, one child, or an array. Use
`childrenToArray(children)` to get a flat array without empty values.

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
not by call order. Two `useLocal` calls with the same name in one component fail (`UI_LOCAL`). A cell
that a render does not declare is dropped when that render commits, so a conditionally declared
cell loses its value while the condition is false.

A component's identity is its module and declaration name (`pages/lab/index#Inputs`), fixed at build
time, so minified release builds and hot swaps agree. Declare components at the top level of a module;
keying by `Component.name` fails the build (`UI_KEY`).

## testId

Every element takes `testId`, the name automation (`lxdev`, the test runner) uses to find it. It has
no visual or accessibility effect.
