# GPU authoring contract

## Package layout

`packages/lingxia-view`:

- `contracts/` — `schema.json` (tags, fields, CSS-named style, elements),
  `api.d.ts` (hook and data types) and `generate.mjs`. Every generated
  artifact comes from here; `generate.mjs --check` and the contract tests fail
  on drift in either direction.
- `compiler/` — TSX audit, JSX source annotation, Tailwind lowering,
  stylesheet loading and the module banner.
- `src/runtime/` — render state, JSX arena, hooks, the action bridge, element
  resolution and wire encoding. `src/style/` — class cascade and author-style
  lowering.
- `src/index.js` is the public entry. `src/internal.js` (`@lingxia/view/internal`)
  is the host protocol for the native session and tests; pages never import it.
  The compiler injects only the JSX factory.

## Elements and props

The schema's `elements` table is the single admission contract: the generated
d.ts, the compiler audit and the runtime all read it. Each element lists its
children kind (any, content, text, none), its props, an optional binding
(`bind` or `value` plus `onInput`/`onChange`) and the style groups it renders.
`view` is a column. `disabled` replaces `enabled`. Grid tracks, line clamps
and image fit are style (`gridTemplateColumns`, `lineClamp`, `objectFit`).

`<text>` children may be strings, `<span>` and `<link onClick>` runs (≤256): nested
spans flatten into styled runs of one paragraph. Runs accept only text paint
(color, size, weight, family, italic, decoration, numeric variants, background);
links default to the accent color with an underline, take `active:` paint and
hit-test only their line pieces. A link may hold spans; its pieces share one
handle, press together and read as one link. Rich text selects as one paragraph.
Any element may set `role` (heading with `headingLevel`, list/listitem, dialog,
alert, status), `aria-hidden` (removes the subtree from semantics, not
paint) and `aria-live`; alert and status announce assertively and
politely by default; an alert also speaks when it appears, a status only when it changes. Controls keep their own role. The Android adapter reads the
role and key names from the generated `GpuSemantics` class.
The projection nests published nodes under their nearest published layout
ancestor (a role, or a scroll); unpublished wrappers collapse and reading order
is source order. Android's `findAccessibilityNodeInfosByViewId` does not search
node providers, so accessibility-service automation must walk the tree by test
id; `lx.automation` queries the projection directly.
Composed buttons and checkboxes may contain noninteractive content; inline
text among element children becomes text nodes. Their accessible name comes
from descendant text unless `aria-label` is given. Empty strings,
booleans and nullish children render nothing, so `{text && <x/>}` is safe.

Editors enforce `maxLength` natively (insertions are cut, existing text kept), clamp
and snap `min`/`max`/`step` when they blur or submit and report it as input, and
hold only ISO values with `type="date|time"`. The Android adapter maps the
attributes through the generated `GpuInput` class.

## Hooks and View

- `useView()` returns the page View; components receive props only and type
  data through props when they need it.
- `useLocal(name, initial)` holds JSON state. `set` on a prepared candidate's
  cell applies when it commits; `set` on an unmounted cell throws with its scope.
- `useRequest(name, action, params?)`: starts after commit, repeats on new
  params or catalog. A reload keeps the previous `value` and sets `stale`.
- `useStream(name, action, { onData? })`, `useChannel(name, action, { params, onData? })`
  and `useLeaveGuard(name, { enabled })` own committed resources.
- Resource errors are `{ code, message }`. Actions are references from
  `view.actions`; string action names are not accepted. An absent reference
  waits only while the catalog is empty.
- Direct hooks in a keyed list row factory require `itemKeys`; otherwise move
  them into a keyed component.

## Diagnostics

The compiler reports every problem in a page at once, as
`file:line:col: CODE: message`, and writes no module when any exist:

- `UI_TAG`, `UI_PROP`, `UI_STYLE` for elements, props and inline style
  properties not admitted on that element;
- `UI_CLASS_UNKNOWN` — not a Tailwind utility; names the nearest utility the
  renderer supports when one is a few edits away;
- `UI_CLASS_UNSUPPORTED` — Tailwind CSS the native renderer cannot draw
  (transitions, skew/3D transforms, unsupported selectors);
- `UI_CLASS_ELEMENT` — a supported utility on an element that ignores it, or a
  state variant on an element that never enters that state.

Class analysis follows literals, conditional branches, const aliases and
imported maps to their defining module, without executing author code.
Runtime errors carry the JSX chain (`at <tag> (file:line:col)`, `at Component`).

## Module compatibility

Compiled modules start with the `LingXia UI module` banner declaring `uiAbi`,
`minField` (the highest field the runtime can emit), required engine features
and the compiler version. Native validates it before running author code.
Changing the ABI or features needs both JS banner tests and native rejection
tests.
