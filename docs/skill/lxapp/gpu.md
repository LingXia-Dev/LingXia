# GPU Views (experimental)

Use `index.ui.tsx` with `"renderer": "gpu"` in its page JSON and `@lingxia/ui`
as a View dependency. Native navbar/tabbar stay owned by the host. GPU Views
have no DOM or browser globals; Logic still owns `lx.*` and page data updates.
The current GPU host adapter is Android; engine dependencies alone do not
provide iOS/Harmony GPU support.

Android GPU text follows the system font-size preference, including
`text-[17px]`; these are author font units, not fixed physical pixels. Use
`min-h-*` with padding for text controls that must grow; `h-*` remains a fixed
layout height and can clip enlarged text. Do not multiply font sizes by display
density or the system preference in View code.

Import types and hooks from `@lingxia/ui`. Return one layout node from a page
at the page root. Child components may return null/undefined/boolean to render
nothing while retaining their own state; fragments remain unsupported. Forward
`Props` through components normally:

```tsx
import { useLocal, type Props, type View } from '@lingxia/ui';

function Card(props: Props) {
  return <column {...props}/>;
}

type Actions = { save(value: string): Promise<void> };
type Data = { title: string };

export function render({ data, actions }: View<Actions, Data>) {
  const draft = useLocal('draft', '');
  return <Card className="gap-3 p-4">
    <text className="text-lg font-semibold">{data.title}</text>
    <input bind={draft} accessibilityLabel="Name" placeholder="Enter your name" onSubmit={actions.save}/>
  </Card>;
}
```

`useLocal` uses an explicit name scoped to component/list identity, not React
hook position. Keep semantic list keys stable across reorderings. Data is
recursively read-only, including arrays; call actions to change Logic state.
Local values are also recursively read-only; replace them with `local.set(...)`.
Use JSON-shaped locals: undefined object fields are omitted; undefined roots or
array entries, functions and class instances are invalid. Declare each local name
once per component render. Custom hooks take a caller-supplied namespace and
prefix internal names (`name + '/value'`) so repeated hook instances stay separate.
Request/stream results and stream/channel messages are recursively readonly too.
Prefer `<List items={items} getKey={item => item.id} renderItem={item => <Row item={item}/>}/>`
for typed lists; it supplies count and keys together.
Direct hooks inside a keyed `row` factory require `itemKeys` before execution.
Alternatively return a keyed component and put its hooks inside that component.
Unkeyed row factories have positional state. Virtualization retains offscreen
row locals/resources; use complete `itemKeys` so removed items can be disposed.
Use `ReadonlyData<T>` for components that consume replicated records without
mutating them. Share data/action payload types between View and Logic.
`bind` owns value and change
handling: do not combine it with `value`, `onInput` or `onChange`.

For typed page actions, pass the action reference to resource hooks:
`useRequest('profile', actions.loadProfile, { id })` infers its payload and
result; `useStream('reply', actions.onSend, { onData })` infers chunks and the
payload required by `stream.start(...)`. Pass registered page actions, not
arbitrary callbacks; the resource owns starting/canceling work after commit.
Omitting a payload preserves omission; explicit `null` remains null.

Author complete Tailwind class names. The official Tailwind compiler expands
them at build time; the native adapter supports a subset of CSS declarations.
Unsupported/unknown utilities fail diagnostics, not silent fallback. Use
`<view className="flex flex-col md:flex-row">` for responsive flex layouts.
`view` defaults to row; `row`/`column` keep their default axes, and classes can
override them. Flex direction/display apply to these flex containers only;
wrap content inside scroll/grid/stack instead of changing their layout model.
Inline `style` overrides utilities and can hold calculated geometry.
Font weight inherits like font size/family/style; use `font-normal` to reset it.

Literal classes, conditional branches, imported/local constant aliases and
object maps receive build diagnostics. Helper calls and dynamically constructed
classes are not fully traced; runtime checks remain. Avoid constructing utility
names from fragments.

Components receive undefined/one child/an array for zero/one/multiple children.
Use `childrenToArray` to inspect flattened, nonempty children. Accept only props
your component uses or forwards; use `LeafProps` for leaf components. Components
with separate root/content nodes should expose separate styling slots.
Explicit JSX children override `props.children`. Null, undefined and booleans
render nothing; arrays flatten. Text accepts text children; inputs, images,
vectors, plots and spacers are leaves. Lists receive children through `row`.

Give images `alt="Description"` when they convey information, or `alt=""` for
decoration/repeated labels. Empty alt removes the image from the semantic tree,
including when it has a testId; it does not change painting or layout. Omitting
alt retains legacy unlabeled-image behavior. Alternative text is limited to
1024 UTF-8 bytes and cannot be whitespace-only.

Images expose `onLoad` and `onError` after presentation. Keep the same `src`
and change `retryKey` (a string, at most 256 UTF-8 bytes) for an explicit retry;
ordinary rerenders never retry settled failures. Use local state for loading,
failure and fallback UI. A shared source/retry pair shares one request; each
mounted image receives its outcome once. Removed requests are canceled and
late results cannot update replacements. Resource authorization/cache remain
host-owned. Give editors an `accessibilityLabel`; placeholders are hints, not names.

Keep bundle and native host capabilities compatible. `UI_COMPAT` means the
bundle requires a newer or different native UI runtime; rebuild/install the
host instead of retrying a page render. Run package type checks as well as
`lingxia build`: the bundler transpiles TSX and does not replace TypeScript's
semantic checker.
