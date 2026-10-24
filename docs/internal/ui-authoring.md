# GPU authoring contract

The compiler and JS runtime share intrinsic property admission in
`packages/lingxia-ui/src/authoring/props.js`. Keep native wire validation
independent. High-level TypeScript declarations live in `src/authoring/index.d.ts`;
the public `src/index.d.ts` reexports them. Rust generates only native style
types in `src/generated/style.d.ts`. The schema drift test checks that artifact.
The authoring type test checks intrinsic names and admitted properties against
the runtime contract, alongside valid and invalid TSX examples.

`View<Actions, Data>` keeps the existing actions-first parameter and optionally
types page data. `ReadonlyData<Data>` recursively protects records, arrays and
tuples to match runtime freezing; read-only components can accept that same
exported type. Keep page data and action payload contracts shared with Logic;
the showcase checks its registered Logic actions against the View interface.
Bound and controlled inputs are mutually exclusive unions;
preserve those unions when deriving textarea/search/checkbox types. Leaf nodes
reject children in types and runtime; text nodes accept text children only.

Prefer `useRequest(name, actions.load, params)` and
`useStream(name, actions.watch, { onData })` when the page has typed actions.
Parameter, message and result types follow the action reference. The string
forms remain available. References must belong to the current page catalog and
match unary/stream mode; arbitrary query functions and stale references fail.
An absent reference may wait only while the catalog is empty. Page actions take
at most one JSON payload.
Omitted/undefined resource payloads remain omitted; explicit null stays null.
Local snapshots are deeply readonly, with replacement through `set`.
Direct hooks in a keyed list row factory require `itemKeys` before execution;
otherwise put the hooks inside a keyed component. Unkeyed factories have
positional state. A returned JSX key cannot retroactively identify factory hooks.

`src/runtime/resources.js` owns request/stream controllers. Runtime registration
still records candidate definitions and commits/disposes them with the page;
controllers must not start work before commit. Tickets reject late completions
after replacement, cancellation or removal. Do not move route authorization or
native transports into these controllers.

`h` consumes `props.children`; explicit JSX children take precedence. Nested
arrays flatten, null/undefined/booleans produce no content. Children forwarded
through component props obey the same leaf/container validation as direct JSX.
Component children are undefined for zero children, the child for one, and an
array for multiple children. `childrenToArray` flattens and filters empty children
without adding layout nodes. A child component may return null/undefined/boolean:
its own locals/resources remain mounted, while descendants are removed on commit.
The page root must produce one node. Fragments fail with `UI_FRAGMENT` at build
time; they are not implemented as implicit layout wrappers.

Each local name must be declared once per component render. Custom hooks take a
caller-supplied namespace and derive names such as `${name}/value`; multiple hook
instances must use distinct namespaces. Candidate discard never changes committed
ownership. Locals accept JSON-shaped values, omit undefined object fields, and
reject undefined array elements/root values. Types reject known non-JSON shapes;
finite-number/depth/plain-object checks remain runtime responsibilities.

Request results, stream results/messages, channel messages and direct action
promise results are deeply readonly on the View side. Logic contracts stay mutable.
`List` maps `items`, `getKey`, `renderItem` and optional `getItemHeight` to the native
list inventory. Its virtualization/lifetime rules are identical to `<list>`.

Component props advertise only forwarded/consumed fields. Leaves use `LeafProps`.
Multi-node components expose separate styling slots: showcase `PageContent`
reserves viewport dimensions, takes root `style`/`className` for viewport paint,
and `contentStyle`/`contentClassName` for safe-area content layout. Utility class
concatenation does not imply override order; inline styles override utilities.

Compiler source analysis uses lexical symbols without loading browser globals.
DOM diagnostics reject unresolved global references, not property names or
local bindings. This is a developer diagnostic, not a security boundary.
Known spread properties use the intrinsic admission contract; unresolved
dynamic properties retain runtime validation.

Class usage analysis follows literal expressions, conditional/logical branches,
local/imported const aliases and named/computed object-map properties. Validate each known complete
utility at its literal source location, including inactive branches. Continue
collecting ordinary strings for Tailwind candidate generation, but do not
diagnose unrelated prose as class usage. Template construction and helper
calls are not yet traced; runtime validation
remains required. Never execute author code to discover possible classes.

Regression tests compile actual TSX to ESM and describe its native records.
Keep source-location failures and successful component composition in the same
gate as runtime transaction tests: `npm test --workspace @lingxia/ui` from
`packages/`.

## Module compatibility

Compiled modules start with a bounded JSON manifest in the `LingXia UI module`
comment. It declares `uiAbi`, `minField`, `requiredFeatures` and `compiler`.
CLI release hardening preserves that first line before minified code; test the
final release artifact, not only the package compiler output.
Requirements cover the bundled runtime's possible output, not just the first
render. The current baseline conservatively requires field108 and bounded JS
jobs; per-page feature minimization is not implemented.

`compatibility.rs` validates new manifests before executing module code and
installs a frozen `__lingxiaUiHost` descriptor. The compiled entry guard also
rejects older hosts that do not install this descriptor. Legacy modules without
the manifest retain their existing validation path. Compatibility grants no
authority and does not replace install signature checks or bridge admission.
Non-QuickJS hosts do not advertise bounded jobs until their adapters implement
them. A dependency on JSC/ArkJS is not a capability declaration.

Changes to the supported ABI/features and compiler requirements need both JS
entry-guard tests and native pre-evaluation rejection tests. Install new bundles
with the matching native library; a successful JS build alone cannot validate
the native compatibility boundary.
