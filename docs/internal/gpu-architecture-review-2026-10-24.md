# Independent GPU architecture and product review — 2026-10-24

## Judgment and review basis

Retain the architecture and continue developing it as an experimental Android
page renderer. It is a substantial implementation with useful transaction,
identity, layout, input and rendering foundations. It is not yet a demonstrated
production-quality cross-platform replacement for WebView page content. Do not
equate successful compilation, hundreds of unit tests, or successful showcase
workflows with that stronger claim.

The highest-value work is to make the contracts between existing layers
consistent, establish repeatable acceptance gates, and extract genuinely shared
semantics before implementing another platform. A wholesale rewrite, a new UI
framework, or moving native navbar/tabbar into the GPU would add cost without
addressing the demonstrated problems. The native-component gallery remains
outside the migration scope.

This review inspected the current dirty working tree. Existing changes were
preserved. Prior review documents and PARITY.md were treated as historical
evidence inventories, not independent proof of completion. No production files
were edited, no builds or device tests were run, and no additional agents were
used. Small read-only Node reproductions were run for findings 1, 3, 4 and 8.
Other findings below explicitly distinguish source-confirmed behavior from
risks requiring measurement or device validation. Paths and line numbers refer
to this working-tree snapshot and will move after edits.

Priority means: **P1** should block general product-readiness claims; **P2** is
important correctness or product work to complete before broad adoption;
**P3** is a narrower authoring issue. These are not security-severity labels.

## Prioritized actionable findings

### Implementation follow-up

The eight findings below are preserved as the original review record. The
follow-up fixes add explicit keyed-factory hook admission, resolved list insets,
omitted payload preservation, recursively readonly local types, inherited font
weight, UI/showcase CI gates, image load/error/retry and transport cancellation,
and member-component admission through the full compiler pipeline. Image
callbacks isolate ordinary handler failures and do not invalidate retained
layout merely because callback handles change. Editors also accept accessible
labels independently of their values.

The independent reviewer rechecked these corrections and the bounded shared-cache
leader takeover without finding another blocker. Download attempts now retain
an abort guard through response-body consumption. Tests cover real socket closure
when the owning future is dropped, plus deterministic shared-leader election.
The latter is not proof of interrupted partial-file takeover on a physical device.
Device follow-up also caught end-anchor drift on repeated renders with asymmetric
insets. Measurement restoration now uses the resolved content height, with a
regression covering repeated renders before and after sibling removal.

Current build and device results are recorded in
[`PARITY.md`](../../examples/lingxia-showcase-gpu/tests/PARITY.md). These fixes do
not establish the later performance, screen-reader, animation or second-platform
acceptance milestones below.

### 1. P1 — Direct row-factory local state follows the index even when JSX has a stable key

**Confirmed by execution.** `packages/lingxia-ui/src/runtime.js:805–813` selects
an index-based scope before invoking `props.row(i)`. It discovers the returned
JSX key only afterward. `useLocal` called directly inside that factory has
already allocated its cell under the index scope; changing scope for the
returned subtree cannot retroactively fix the cell identity.

Reproduction:

```js
let items = ['a', 'b'];
const page = () => h('list', {
  count: items.length,
  row: i => {
    const n = useLocal('count', 0);
    return h('button', {
      key: items[i],
      onClick: () => n.set(n.value + 1),
    }, `${items[i]}:${n.value}`);
  },
});
```

After clicking `a` and committing, the texts are `['a:1', 'b:0']`. Reordering
to `['b', 'a']` produces `['b:1', 'a:0']`. This can move drafts, selections or
resource ownership to the wrong business item. Supplying `itemKeys` before the
factory runs avoids the problem; a keyed child component whose hooks execute
after key resolution also avoids it. The distinction is not apparent from the
public claim that locals follow component/list identity and stable semantic
keys (`docs/skill/lxapp/gpu.md:35`).

**Recommended change:** make the pre-render identity contract explicit. Prefer
a keyed list API that has item keys before invoking row code; the existing
`itemKeys` already provides the needed information. If positional lists remain,
document their positional state semantics. Alternatively prohibit hooks in row
factories unless predeclared keys are present and require a component boundary.
Do not promise that a returned JSX key determines already-executed hooks. Do not
execute the row twice to discover its key.

**Acceptance:** direct factory hooks and child-component hooks retain the right
state/resource through prepend, delete, reorder, eviction/reentry and failed
candidates. A removed keyed item cannot revive another item's cell. Tests must
cover both `itemKeys` and the supported no-`itemKeys` contract.

### 2. P2 — Virtual-list geometry disagrees with Taffy for per-edge padding and borders

**Source-confirmed; current native/device reproduction not run.**
`crates/lingxia-ui/src/list_layout.rs:116–120` derives padding, content width,
viewport and scroll position solely from field 3, the uniform `padding` field.
The end and row anchors repeat that assumption at lines 166–185. By contrast,
`render.rs:2797–2806` resolves individual padding and border edges, and
`render.rs:3144–3147` incorporates borders into resolved content origins.
Tailwind padding lowers to individual edges in
`packages/lingxia-ui/tailwind.mjs:14–19,257`, so even ordinary `p-4` does not
necessarily populate the uniform field read by the list engine.

For a list with positive top/bottom padding and `anchor="end"`, the list engine
targets `totalRowHeight - outerHeight` instead of accounting for those edges.
The last row can remain clipped at the requested end, and trailing padding is
not included in the target. Changing only left/right padding can change actual
wrapping width while leaving `state.width` unchanged at `list_layout.rs:142`;
offscreen measurements then remain based on the previous width. Borders have
the analogous disagreement. The existing session test at `session.rs:1384`
uses `style:{padding:12}` and therefore misses the compiled-utility path.

**Recommended change:** expose resolved content-box/scroll-content geometry
from shared layout and consume it in list reconciliation. Do not add another
independent shorthand/edge implementation to `list_layout`. Measure available
row width and viewport from that resolved geometry, and invalidate measurements
when it changes.

**Acceptance:** identical results for inline uniform padding, equivalent four
edges and equivalent Tailwind classes; asymmetric padding/borders; start/end
anchors; narrow/wide text wrapping; retained scroll; failed-frame rollback.
Compare the last row and actual scroll maximum, not merely that the row exists.

### 3. P2 — Typed request resources change omitted payloads into explicit null

**Confirmed by execution.** `runtime.js:302` uses
`immutable(params ?? null)`, and `runtime/resources.js:65` invokes the action
with that value. Calling `useRequest('load', actions.load)` therefore emits:

```json
{"v":2,"kind":"req","method":"load","cap":"page","params":null}
```

Direct `actions.load()` omits `params` (`runtime.js:67–70`). An optional payload
handler with a JavaScript default argument, or code distinguishing missing from
explicit null, behaves differently through the resource hook despite the typed
overload accepting omission (`authoring/index.d.ts:48`). `useChannel` similarly
normalizes its omitted options payload at `runtime.js:345`; audit that contract
alongside requests.

**Recommended change:** preserve missing/undefined separately from JSON null
when constructing the resource definition, comparison key and action call.
Choose one documented bridge payload rule and apply it consistently to direct
actions and resources. No transport redesign is needed.

**Acceptance:** omitted, undefined, null and object payloads retain their
intended distinct behavior through initial commit, reload, unchanged rerenders
and action-catalog replacement. Include a Logic handler with a defaulted
optional parameter.

### 4. P2 — Local-state types promise mutable values that the runtime freezes

**Confirmed by execution and declarations.**
`authoring/index.d.ts:22` exposes `Local<T>.value` as `T`.
`runtime.js:221–230,246–265` recursively copies/freezes local values. Thus a
type-correct `useLocal('x', {items: []}).value.items.push(...)` throws at runtime.
The recently introduced `ReadonlyData<T>` already addresses the same issue for
replicated View data but is not applied to local reads. Request results and
stream/channel payload types deserve the same review because received JSON is
also frozen; this review directly reproduced the local-state case only.

**Recommended change:** express deeply readonly read values and retain an
explicit replacement/update operation. Design `set` to accept ordinary JSON
values and values derived from readonly snapshots without forcing unsafe casts.
Document JSON-only local state; do not weaken immutability to satisfy the old
type declaration.

**Acceptance:** type tests reject nested object and array mutation, while
immutable updates and readonly component props remain ergonomic. Runtime and
types agree for nested records, arrays, tuples, null and primitive values.

### 5. P2 — Font weight has a deliberate but inconsistent inheritance contract

**Confirmed behavior; design inconsistency, not an accidental regression.**
`render.rs:2726,2770` defaults every text-bearing record to weight 400 instead
of resolving the parent. Font size, family, style, line height and spacing are
resolved through parents immediately above. Consequently the admitted authoring
`<column className="font-bold"><text>Heading</text></column>` silently leaves
the child normal. `render.rs:5348–5367` explicitly tests this non-inheritance.
Passing that test establishes the implementation's chosen rule, not the quality
of the rule for Tailwind authors.

**Recommended change:** inherit weight as part of one shared computed-text-style
pass, with explicit child overrides. Remove the non-inheritance assertion and
replace it with the intended contract. If the exception is retained, it must be
explicitly justified and documented, and unsupported container use should not
silently appear to work. Inheriting weight is the simpler product model.

**Acceptance:** container inheritance, nearest-parent override, child reset,
composed controls, input shaping/caret geometry, text-cache reuse and system text
scaling all use the same effective weight.

### 6. P1 release gate — GPU package and showcase tests are not connected to CI

**Confirmed from repository configuration.** The npm job in
`.github/workflows/ci.yml:1620–1679` explicitly runs several package suites but
never `npm test --workspace @lingxia/ui`. `packages/package.json:11` has an
aggregate workspace test command, but this job does not call it. The package's
own `package.json:13–15` provides the test command, so adding the gate is small.
The workflow's example filters name `examples/lingxia-showcase` and
`examples/lingxia-chat`, but not `examples/lingxia-showcase-gpu`. No GPU showcase
typecheck/Logic-test gate is present in that workflow. Package changes trigger
the general npm job, which is not equivalent to running this package's tests.

There is useful existing coverage: workspace Rust tests and Android unit tests
are configured, and numerous repository-owned device probes exist. This is not
a claim that all GPU tests are missing. It is a specific hole in the public API,
compiler and product-fixture regression gates.

**Recommended change:** add a focused GPU authoring job with package tests,
generated-schema freshness, showcase View/Logic typechecks and the showcase's
injected Logic/parity tests. Add explicit path filters and tests for those
filters. Keep hardware acceptance a separate job or documented release gate;
do not label it passed when hardware is unavailable.

**Acceptance:** intentional failures in a UI package test and a GPU showcase
typecheck fail the relevant PR checks, including a PR changing only that
showcase. The required job cannot pass with zero selected tests.

### 7. P2 product gap — Images have neither author-visible failure state nor an ordinary retry path

**Confirmed contract, with a source-derived latency risk.**
`image_requests.rs:65–100` marks a source started and never requeues it after
completion, regardless of success/failure. The test
`failed_or_removed_sources_retry_only_after_leaving_the_wanted_set` explicitly
codifies this. `ImageProps` in `authoring/index.d.ts:17` exposes only source,
alt and fit; there is no load/error status or retry mechanism. A transient
download/decode failure therefore leaves an image blank until its source leaves
the wanted set or the owner is reset. Re-rendering the same UI does not recover
it, and an app cannot implement a proper fallback from this API alone.

Additionally, `GpuPage.java:22,36–43` has a process-wide two-thread image pool.
`ffi/android/gpu_page.rs:463–471` waits synchronously for an asynchronous load,
and `lingxia-lxapp/src/page.rs:2109` permits a 30-second request timeout. Removing
a request invalidates its result but does not cancel that transport. Two slow
obsolete downloads can therefore delay images for a newly opened page. This
is a source-derived scheduling risk; its wall-clock impact was not measured.

**Recommended change:** retain host-authorized I/O and shared image ownership,
but model loading/ready/failed status with bounded explicit retry and a real
cancellation path for obsolete work. Decide whether status is an image event or
a reusable image resource; do not force authors to alter URLs or briefly remove
nodes to retry. Preserve deduplication and late-result identity checks.

**Acceptance:** offline-to-online retry of the same URL, corrupt bytes, denied
resource, navigation during slow transfers and reload of the same source. New
visible work must not wait for all obsolete transfers to time out. Settled
failure must not produce an idle retry/render loop.

### 8. P3 — Valid member-expression components are misdiagnosed as native tags

**Confirmed by execution.** `compiler.mjs:26–29` tests whether the entire JSX
tag text starts with a lowercase letter. `audit` rejects:

```tsx
import * as components from './components';
export function render() { return <components.Card/>; }
```

with `UI_TAG: unsupported tag <components.Card>`, although this is a component
reference and the runtime supports callable component values. Uppercase
namespace spelling happens to avoid the test.

**Recommended change:** inspect the JSX tag AST. Apply intrinsic-tag admission
only to simple intrinsic identifiers, and let component references follow the
component/type/runtime contract. Continue rejecting actual unsupported native
tags.

**Acceptance:** local/imported function components, namespace/member-expression
components, intrinsic tags and unsupported intrinsic tags receive consistent
compiler and runtime behavior.

## Product and architecture risks that are not proven defects

### Shared accessibility semantics are not yet a complete product contract

The implementation has meaningful work here: stable virtual IDs, hidden image
semantics, composed-control grouping, editable text metadata and actions,
sliders/plots, focus and exploration support. Preserve it. But public inputs
have no explicit accessible label or label relation (`authoring/index.d.ts:30`),
and `GpuTextAccessibility.java:27–40` supplies text and placeholder hints rather
than such a relation. General heading/group/live-region semantics also are not
part of the authored schema. A separate visible text label cannot be bound to
an editor as its semantic name through the current API.

`GpuView.java:1238–1432` derives roles, actions and hierarchy largely from tags
and JSON on Android. A second platform should not independently reconstruct
those product semantics. Introduce a small shared semantic projection with
stable identity, role/name/value/state/actions, relationships and geometry;
leave platform API calls and speech conventions in the adapters. Keep native
chrome semantics native.

Do not claim TalkBack usability from UiAutomator node presence. Require actual
TalkBack exploration and sequential navigation through Home, ToDo, dialogs,
editing, dynamic stream updates, disabled controls, errors, scrolling and page
transitions, with explicit spoken names/states and focus continuity recorded.
The exact current spoken behavior was not tested by this review.

### Main-thread presentation and the shared GPU lock need measured acceptance

`GpuView.java:683` presents from a Choreographer callback.
`android/gpu_surface.rs:147,176` holds one global presenter-registry mutex during
worker preparation and UI-thread presentation. `gpu.rs:199–270,288–345` also
holds the shared device-context lock around substantial rendering and surface
operations, including acquisition and submission. Thus a worker preparing one
page can delay another page's UI-thread presentation/suspend/detach. The lock
order is documented and sensible for correctness; that does not establish
bounded UI latency.

Do not rewrite threading on speculation. First capture lock wait, acquire,
submission and Android frame timelines under concurrent page/surface work.
If unacceptable, shorten registry critical sections by looking up stable
presenter handles and consider a render-executor-owned submission/presentation
protocol with explicit surface lifetime tickets. A mutex-per-presenter alone
does not remove shared-device serialization or surface-destruction obligations.

The existing performance script records useful CPU-wall metrics but does not
enforce a product latency/FPS budget
(`tests/gpu/scripts/verify-performance.mjs:43–94`). It also relies on per-frame
logs. Require repeated release-mode traces and matched WebView/GPU workloads;
settled screenshots and whole-black-frame detection cannot establish frame
pacing, GPU time or power efficiency.

### Resolve presentation, semantic and callback revisions separately

`ViewSession` commits candidate records/resources after pixel preparation, while
Android publishes input/accessibility metadata after successful presentation.
Animations have an explicit presentation acknowledgement. These are useful
separate phases, not inherently a defect. Preserve the distinctions in a
documented frame transaction rather than extending one overloaded generation.

Android currently compares entire serialized node arrays for content changes
(`GpuView.java:691,969–975`). Regenerated callback handles can make an otherwise
unchanged semantic tree look changed. A shared semantic revision/diff would
avoid unnecessary accessibility subtree notifications and repeated large
main-thread JSON comparisons. Benchmark before replacing the entire wire
representation; the existing primitive buffer and compatibility guard are
valuable.

### Virtualization and resource budgets need an explicit long-session policy

The native stack has many useful limits: record, string, vector, editor, image,
texture and execution budgets. Keep those. JS row-local cells are retained for
visited live rows (`runtime.js:650–655,838`), and their owned resources can stay
alive while rows are evicted. That can be correct for drafts and stream
lifetimes, but it makes virtualization a paint/layout optimization rather than
a bound on all page state or background work.

Define which resources persist while offscreen, how semantic removal is
detected without a full key inventory, and what page/app memory limits apply.
Audit actual JS engine heap limits and aggregate cache/device allocations.
This review did not demonstrate an OOM or a leak and does not claim one.
Long-session churn, many distinct rows, failed downloads, repeated reloads and
concurrent pages should be acceptance workloads.

### Apple/Harmony readiness requires a platform contract, not backend feature flags

`module.rs` uses engine-specific module/job operations; non-QuickJS paths have
different evaluation/job behavior. `budget.rs:24–30,64–65` explicitly lacks the
same preemption on Apple/Harmony. `compatibility.rs:16–27` correctly advertises
`bounded-jobs` only where implemented, and current bundles require that feature
(`compiler/compatibility.mjs:4`). Do not weaken this admission check merely to
make another platform load a bundle.

A second platform needs a chosen JS execution/isolation strategy, explicit
surface/presentation lifecycle, IME/composition, system text scaling, semantic
projection and authorized resource adapter. Engine/backend dependencies are not
that adapter. There is no evidence here of working iOS or Harmony GPU pages.

## Target boundaries: retain, rework, remove

| Area | Retain | Focused rework/removal |
| --- | --- | --- |
| Authoring | TSX function components, explicit named local state, typed page action references, View/Logic separation | Pre-render list identity; deeply readonly reads; consistent optional payloads; AST-based component admission |
| Styling | Official pinned Tailwind compiler, build-time lowering, explicit unsupported-feature errors, inline precedence | One computed typography contract and one resolved box geometry contract; remove the font-weight exception; test semantic equivalence rather than only emitted fields |
| Shared engine | Taffy, Parley, retained display lists, editor ownership, CPU reference, GPU rendering, bounded candidate transactions | Extract computed style/content geometry and shared semantic projection where they prevent current duplication; keep caches owned by the right session/device |
| Protocol | Append-only fields, independent native validation, ABI/capability admission | Generate field identifiers/accessors to reduce magic-number drift; add declaration/schema freshness checks; do not share one validator implementation across the trust boundary |
| Resources | Host/facade authorization, immutable decoded images, source/token lifetime checks, committed request/stream resources | Missing payload preservation, explicit image failure/retry/cancel, documented offscreen resource policy |
| Android | Native chrome, surface/IME/system accessibility adapters, stable IDs, lifecycle tickets, explicit reload of poisoned sessions | Shared semantic rules instead of parallel tag inference; measured lock/JSON costs; adapter state-machine cleanup only with transition tests |
| Animation | Presentation-bound timeline, visibility suspension, semantic identity, candidate rollback | Keep production authoring gated until physical scheduling, transformed geometry and both rendering paths pass acceptance; do not add JS intervals or page-specific effects |
| Examples/tooling | Independent GPU showcase and repository-owned probes; retain original React reference | One reproducible suite manifest/driver and artifact schema; remove obsolete harness assumptions from active gates; keep historical evidence clearly separate |
| Documentation | Short skill entrypoints and subsystem invariants | Replace accumulated chronological acceptance logs with a current capability matrix, invariants and linked run artifacts; preserve useful history outside the normative contract |

No recommendation here calls for replacing Taffy/Parley, merging Logic and View,
building a general browser CSS engine, exposing internal crates to app authors,
or migrating native-component demos. Any such proposal should demonstrate a
specific problem these smaller changes cannot solve.

## Staged implementation plan and acceptance gates

### Stage 1 — Close reproducible contract defects and install the regression gate

Dependencies: none beyond the existing toolchain. Fix findings 1–4 and 8 in
small cohesive changes; decide and fix finding 5 with the corresponding
semantics test. Add finding 6's CI coverage immediately so these changes remain
protected. Keep list geometry source-of-truth extraction narrow.

Exit: focused regression/type tests pass; generated schema is fresh; package
suite and showcase View/Logic checks run in CI; one physical Android check of
keyed row editing/reorder and padded end anchoring passes on a matching APK.
No claim that a full product gate is satisfied by this stage.

### Stage 2 — Define and complete the shared product contracts

Depends on Stage 1's identity and geometry contracts. Add input labels and
shared semantics, explicit image error/retry/cancel, and a documented offscreen
resource policy. Document candidate commit versus presentation acknowledgement,
surface generation, input-token and connection-generation transitions. Factor
Android policy into the shared core where a second adapter would otherwise
duplicate behavior; avoid moving Android API mechanics into Rust.

Exit: lifecycle/resource race tests cover removal, late completion, failed
candidate, failed presentation, hide/show, reload and disposal. Real Android
IME and TalkBack acceptance covers named forms, navigation, errors and dynamic
content. Old failed image work cannot starve a newly opened page.

### Stage 3 — Make Android product acceptance repeatable

Depends on a stable Stage 2 semantic/lifecycle contract. Create a repository
manifest of active scenarios, fixtures, required permissions, setup, cleanup,
expected assertions and artifact locations. Each result records source/build
identity, APK hash/version, device/OS, viewport, text scale, theme/language and
backend. Distinguish pass, fail, skipped and unverified; do not silently reuse
historical smoke-harness runs.

Exit: a clean machine can install the matching APK and rerun a core suite for
Home/ToDo, editing, streaming/channel resources, native chrome, navigation,
recovery, rotation, text scale and background quiescence. Visual comparisons
use fixed data and matched state. Fault scenarios include network/resource
failures and stale callbacks. Native device features retain separate real-device
gates; excluded gallery demos stay excluded.

### Stage 4 — Establish performance and memory acceptance, then optimize

Depends on Stage 3's repeatable workloads. Agree concrete target-device latency,
frame pacing, memory and idle-power budgets before optimizing. Capture cold and
warm runs, sustained list/stream/edit workloads, repeated navigation and
concurrent surfaces. Use Android frame timelines/Perfetto or equivalent actual
presentation evidence alongside current CPU timing counters; retain thermal
state and repeated-run distributions.

Exit: budgets pass in release builds, not merely metric-presence assertions.
Show UI-thread stalls and missed frames, plus peak/steady memory and hidden-page
wakeups. Optimize registry locks, scene construction, main-thread JSON and cache
ownership only where traces justify it. Rerun correctness/semantics after each
optimization. Do not describe CPU wall time as GPU time or display FPS.

### Stage 5 — Enable animation and prove a second platform

Animation depends on Stage 3's reproducible visual/lifecycle gate; platform work
depends on Stage 2's shared contracts. These can progress independently once
their prerequisites exist. Keep animation capability gated until step/continuous
effects, insertion/eviction/reentry, pause/resume, interruption and final-frame
presentation are physically verified on both GPU and fallback paths.

For the second platform, select one small vertical slice: page bootstrap,
text/form/list, image resource, native chrome integration, actual presentation,
hide/show/reload and accessibility. Resolve module evaluation, microtask draining
and execution-budget capabilities before advertising support. Reuse the shared
engine and semantic policies; test platform mechanics on actual hardware.

Exit: the same contract suite passes on the second adapter, its capability
manifest honestly describes its guarantees, and platform-specific unverified
device features remain marked as such. Only then broaden migration/support.

## Limits and evidence

This was a repository/code/architecture review, not exhaustive line-by-line
verification of every backend, shader, unsafe block, SDK path or example action.
Representative paths across authoring, compilation, state/resources, schema,
session transactions, layout/text/input, retained rendering, composition,
Android integration, facade authority, animation and testing were inspected.
The review read the applicable skills and relevant internal ownership/bridge/
layout contracts. It did not independently rerun the reported 297 shared UI,
188 Android or 186 JS tests, workspace checks or device evidence.

Read-only Node observations were:

- Request without payload produced explicit `params:null`.
- Keyed JSX with direct row-factory locals produced `a:1,b:0` before reorder and
  `b:1,a:0` afterward.
- Mutating a nested local array threw `Cannot add property 0, object is not
  extensible`.
- A lowercase namespace member component was rejected as an unsupported tag.

The font-weight exception and list box-model disagreement were established
from source and existing tests; no current native binary was used as proof.
Image cancellation latency, aggregate memory, cross-page GPU-lock latency,
screen-reader speech, frame pacing/power and second-platform behavior remain
unmeasured. This review makes no claim of an exploitable security issue,
demonstrated memory leak, proved GPU performance regression, or working
iOS/Harmony GPU support.
