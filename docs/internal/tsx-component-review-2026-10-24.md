# TSX and component design review — 2026-10-24

## Conclusion

Keep TSX, pure function components, named local state, transactional commits,
typed Logic action references, and the Tailwind-to-native adapter. The next
priority is a coherent authoring contract and a reusable component layer, not
another renderer rewrite or a larger intrinsic-tag catalog.

This review found five reproducible type/runtime mismatches and one component
API consistency problem. The earlier GPU review corrections remain valid;
they did not establish completeness of the component language.

## Findings

### 1. P1: duplicate local names silently alias cells, including incompatible types

`packages/lingxia-ui/src/runtime.js:240–256` looks up an existing candidate cell
before considering the new initializer. Unlike resource registration, it does
not reject a second declaration in the same component scope.

```tsx
const count = useLocal('state', 0);
const draft = useLocal('state', '');
draft.value.toUpperCase();
```

TypeScript accepts this; the second value is actually a number. The real compiler
also accepts it, and the rendered page throws. Two independently authored custom
hooks that internally choose the same name can silently share state even when
their types match, making this more than an unfortunate error message.

Require unique registrations within a component render. For composable custom
hooks, define an explicit caller-supplied namespace/owner contract; do not
silently reinterpret duplicate declarations as a shared-store API. Test duplicate
names, custom-hook instances, candidate discard, and conditional registration.

### 2. P2: one-child component props are always delivered as arrays

`runtime.js:364–371` stores children as an array, and `:419` passes that array
directly to every function component. An explicitly supplied `children` prop is
also wrapped. The public declarations allow a single element.

```tsx
function Forward({ children }: { children: JSX.Element }) {
  return children;
}
<Forward><text>One</text></Forward>
```

This passes TypeScript and the actual compiler, then fails with
`UI_ROOT: expected one node`. A component cannot safely rely on its declared
single-child contract. The existing forwarding test inserts a `<column>`, which
flattens the array and therefore misses this case.

Specify the public shape for zero/one/many children independently of the internal
arena representation. Prefer undefined/single child/array respectively, with a
documented children-normalization helper for components that inspect children.
Keep explicit JSX-child precedence. Do not repair transparent components by
silently inserting layout nodes.

### 3. P2: nullable child components type-check but fail the whole candidate

`runtime.js:407–419` requires every component result to be a branded node, although
the type checker accepts `JSX.Element | null` components and ordinary JSX children
already admit null/false/undefined.

```tsx
function Optional(): JSX.Element | null { return null; }
<column><Optional/></column>
```

The compiled module fails with `UI_ROOT`. The current one-node restriction is
documented; this finding is a public type/behavior mismatch and a composition
limitation, not a claim that fragments were promised.

Support empty results from child components, with explicit local/resource
lifetime rules: an invoked component returning null can retain its own state,
while descendants are absent. Keep the page-root layout contract distinct.
Fragments can follow separately after identity, sibling-key and lifecycle rules
are specified; a hidden wrapper would change flex/grid/stack behavior.

### 4. P2: readonly resource results/messages are still typed as mutable

`src/authoring/index.d.ts:9–11,47–53` exposes nested `T`/`R` directly in request
results, stream results, stream/channel messages and callbacks. `runtime.js:178`
and `:183` recursively freeze incoming bridge values.

For a typed action returning `{ items: string[] }`,
`useRequest('load', actions.load).value?.items.push('two')` passes TypeScript,
but the actual fulfilled resource throws when its array is mutated.

Apply `ReadonlyData` consistently at the View-side boundary, including event
callbacks and async iteration, while retaining useful inference from shared
Logic contracts. Merely marking the outer property `readonly` is insufficient.
Do not unnecessarily make Logic's internal working data immutable.

### 5. P2: Local<T> admits values outside its runtime value domain

`src/authoring/index.d.ts:22,46` accepts unconstrained `T`; `runtime.js:221–229`
admits only finite JSON-shaped values. `useLocal('date', new Date())` passes
TypeScript but throws `UI_LOCAL: expected JSON value`. Optional properties
explicitly set to undefined have the same underlying mismatch.

Specify and type the local value domain. Either keep JSON-only locals and reject
known unsupported shapes at type checking, or deliberately introduce separate
semantics for non-serializable local values. Do not weaken the existing freeze,
transaction and bridge guarantees as an incidental fix. Optional/undefined
normalization must be an explicit decision.

### 6. P2: component prop declarations overpromise forwarding and layout ownership

`shared/components/icon.tsx:23` accepts `Props & { name }`, but forwards only
className/testId; `style` and children are silently ignored. Several page-local
`Header`/`Results` components similarly intersect with all `Props` and discard
advertised styling attributes. Examples are in `pages/device/index.ui.tsx:14–20`
and `pages/system/index.ui.tsx:21–27`.

`shared/components/page.tsx:12–23` forwards styling to an outer viewport wrapper
but inserts author children inside another column. Thus a caller's `gap` or
alignment styles affect the inset scaffold rather than the content. Current
call sites primarily use background/text styling; this is an API-design hazard,
not a demonstrated current-page visual regression.

Use explicit component props: leaf components should not accept children they
ignore; accepted style props should be forwarded or deliberately documented as
owned. Separate viewport/background props from content layout props where there
are multiple styled nodes. Avoid inheriting `Props` solely for convenience.

## Recommended component architecture

1. **Language contract:** define component result/children/key semantics, typed
   component-prop utilities, readonly values and local/resource ownership first.
   Add positive TSX type tests paired with compiled runtime tests; a successful
   transpile is not evidence that the component contract works.
2. **Native primitives:** retain the small existing layout/text/input/image set
   and independent native validation. Preserve specialized scroll/list/grid
   layout semantics. Do not advertise arbitrary CSS/DOM compatibility.
3. **Reusable UI components:** provide safe-area content, text/icon buttons,
   field labels/errors, cards, settings rows and loading/empty/error states in
   TSX over primitives. Separate semantic behavior from product-specific visual
   variants. Shared contracts should live outside showcase page files; example
   styling can remain app-owned until multiple real consumers justify promotion.
4. **Data-driven list authoring:** offer a typed items/getKey/renderItem facade
   over the existing count/itemKeys/row engine contract. Compute keys and indices
   in one place instead of asking every author to synchronize three inputs.
   Explicitly retain the virtualized offscreen-state/resource policy and budgets.
5. **Styling/DX:** preserve Tailwind's actual CSS order, with explicit component
   variants and documented root/content styling slots. Appending a class string
   is not a defined override API. Add source/component context to runtime
   diagnostics so a `UI_ROOT` failure identifies the offending component.

Do not introduce a universal polymorphic component or a large design-system
dependency before these contracts stabilize. Keep native chrome host-owned.

## Verification and limits

Review-only: no production implementation was changed. The executable probe is
`target/tsx-component-review.mjs`; results are in
`target/tsx-component-review-results.json`. Its virtual TypeScript program
reports zero diagnostics for the invalid-at-runtime cases. It reproduces local
aliasing, invalid JSON locals and frozen request results, and runs the forwarding,
nullable and duplicate-local cases through `buildPages` and the generated ESM.

Source review also covered authoring declarations/admission, compiler stages,
resource controllers, styling contracts, existing type/compiler tests and
representative showcase components. No device tests or full suite were rerun;
these are authoring/runtime probes, not GPU raster or accessibility-speech proof.

## Remediation

All six findings are addressed: duplicate-local diagnostics and hook namespaces;
zero/one/many children and normalization; empty child results with transactional
ownership; readonly View result/message types; checked JSON local shapes and
optional object normalization; precise showcase props and root/content slots.
The typed `List` facade derives count/keys/rows together. Component errors include
name and runtime identity. Regression coverage pairs compiled TSX, runtime state/
resource lifecycle tests, and positive/negative TypeScript examples. Reorganization
validation is recorded separately; earlier device evidence is not proof for a
new main-based build.
