# View authoring contract

What a View page may contain and how the build checks it before native code
runs it. Renderer internals: [view-renderer.md](view-renderer.md); styling:
[view-styling.md](view-styling.md); motion: [view-animation.md](view-animation.md);
wire bounds: [view-engine-limits.md](view-engine-limits.md).

Invariants:

- `contracts/view/schema.json` is the single admission contract. The compiler
  audit, the runtime, the native decoder and every platform adapter read
  generated copies of it; none keeps its own list of tags, props or styles.
- A page that fails the audit produces no module.
- Native validates a module's banner before running any author code.

## Contract package

`contracts/view/` holds `schema.json` (tags, fields, CSS-named style,
elements, limits, roles), the TypeScript hook and data type declarations, and
the code and reference generator (Rust, JS, Java, Swift and ArkTS outputs, plus
`docs/view/reference` and [view-engine-limits.md](view-engine-limits.md)).
The contract generator's `--check` mode and the contract tests fail on drift
in either direction.

The `@lingxia/view` package:

| Area | Owns |
| --- | --- |
| Compiler | TSX audit, JSX source annotation, Tailwind lowering, stylesheet loading, the module banner |
| Runtime | render state, JSX arena, hooks, the action bridge, element resolution, wire encoding |
| Style | class cascade and author-style lowering |
| `@lingxia/view` | the public entry |
| `@lingxia/view/internal` | the host protocol for the native session and tests; pages never import it |

The compiler injects only the JSX factory into a page.

## Elements and props

Each entry in the schema's `elements` table lists its children kind (`any`,
`content`, `runs`, `none`), its props, an optional binding (`bind`, or `value`
plus `onInput`/`onChange`) and the style groups it renders. `view` is a
column. Controls take `disabled`; there is no `enabled`. Grid tracks, line
clamps and image fit are style (`gridTemplateColumns`, `lineClamp`,
`objectFit`), not props.

Empty strings, booleans and nullish children render nothing, so
`{text && <x/>}` is safe. Composed buttons and checkboxes may contain
noninteractive content; inline text among element children becomes text
nodes. A button or checkbox holding only another control fails with
`UI_BUTTON`: it would be a second target for one action.

### Rich text

`<text>`, `<span>` and `<link>` take runs: strings, `<span>` and
`<link onClick>`. Nested spans flatten into styled runs of one paragraph.

- Runs accept only text paint: color, size, weight, family, italic,
  decoration, numeric variants, background.
- Links default to the accent color with an underline, take `active:` paint
  and hit-test only their line pieces. A link may hold spans; its pieces share
  one handle, press together and read as one link. Links do not nest.
- Rich text selects as one paragraph.
- A popover or dialog a run's component returns is lifted out of the
  paragraph and drawn in the top layer.

### Semantics

Any element may set `role` (the schema's `options.role`, listed in
`docs/view/reference/semantics.md`; `heading` takes `headingLevel`),
`aria-hidden` (removes the subtree from semantics, not paint) and `aria-live`.
Controls keep their own role. `alert` announces assertively and `status`
politely by default; an alert also speaks when it appears, a status only when
it changes. A composed control's accessible name comes from descendant text
unless `aria-label` is given.

The projection nests published nodes under their nearest published layout
ancestor (a role, or a scroll); unpublished wrappers collapse and reading
order is source order. The Android adapter reads role and key names from
generated Java constants. Android's `findAccessibilityNodeInfosByViewId`
does not search node providers, so accessibility-service automation must walk
the tree by test id; `lx.automation` queries the projection directly.

### Editors

Editors enforce `maxLength` natively (insertions are cut, existing text kept),
clamp and snap `min`/`max`/`step` on blur or submit and report the result as
input, and hold only ISO values with `type="date|time"`. The Android adapter
maps the attributes through generated Java constants.

## Hooks

Signatures live in the `contracts/view/` TypeScript declarations; these are the rules the types
cannot state.

- `useView()` returns the page View, `useEnv()` only its environment (a
  component reading only that re-runs when a field it read changes, not on
  data changes). Memo records keep the environment fields their subtree read,
  by page code or by class lowering; a component nothing reads env in survives
  any environment change. Components that need typed page data take it through props.
- `useLocal([name,] initial)` holds JSON state, keyed by name or by call
  order. `set` on a prepared candidate's cell applies when that candidate
  commits; `set` on an unmounted cell throws `UI_LOCAL` naming its scope.
- `useRequest(name, action, params?)` starts after commit and repeats on new
  params or a new action catalog. A reload keeps the previous `value` and sets
  `stale`.
- `useStream`, `useChannel` and `useLeaveGuard` own committed resources:
  nothing starts for a candidate render that is discarded, and unmounting
  disposes them.
- Resource errors are `{ code, message }`.
- Actions are references from `view.actions`; string action names are not
  accepted. Reading an action before the catalog arrives yields `undefined`
  and the hook waits; once the catalog is present, an unknown name fails.
- Hooks called directly in a keyed list row factory require `itemKeys`;
  otherwise move them into a keyed component (`UI_LIST`), because row state
  is matched to rows by those keys.

## Compiler diagnostics

The compiler reports every problem in a page at once, as
`file:line:col: CODE: message`, and writes no module when any exist:

| Code | Meaning |
| --- | --- |
| `UI_TAG`, `UI_PROP`, `UI_STYLE` | element, prop or inline style property not admitted on that element |
| `UI_CLASS_UNKNOWN` | not a Tailwind utility; names the nearest supported utility when one is a few edits away |
| `UI_CLASS_UNSUPPORTED` | Tailwind CSS the native renderer cannot draw (for example skew or 3D transforms, unsupported selectors) |
| `UI_CLASS_ELEMENT` | a supported utility on an element that ignores it, or a state variant on an element that never enters that state |

Class analysis follows literals, conditional branches, const aliases and
imported maps to their defining module, without executing author code.
Runtime errors carry the JSX chain (`at <tag> (file:line:col)`,
`at Component`).

## Module compatibility

A compiled module starts with a `// LingXia UI module; {…}` banner declaring
`uiAbi`, `minField`, the required engine features and the compiler version.
Native (`lingxia-view`) validates it against its own `uiAbi`,
`maxField` and features before running author code, so an old host refuses a
newer page up front instead of failing mid-render.

- `minField` is a lower bound: the highest non-ignorable field the page's JSX
  props, literal style keys and class rules name on an element that takes it,
  never words in strings or comments.
- The encoder is the exact check: a non-ignorable field past the host's
  `maxField` fails that render with `UI_COMPAT`, which the host shows as
  "update the app".
- Mark a new field `"ignorable": true` in `schema.json` only when it is
  accessibility-only: on a host below it, the encoder leaves it out with one
  `UI_COMPAT` warning. A field that changes layout, paint or behaviour (roles
  included) is never ignorable, because dropping it would draw a different
  page than the author wrote.

Changing the ABI or features needs both JS banner tests and native rejection
tests.
