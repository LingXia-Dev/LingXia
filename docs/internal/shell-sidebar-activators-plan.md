# Desktop Sidebar, Pins, and Runtime Activators Plan

Status: implemented on `feat/shell-ui-spec`; macOS compilation remains a PR CI gate.

Target branch: `feat/shell-ui-spec` / PR #126.

## Purpose

Unify the desktop shell sidebar on Windows and macOS without making either
platform implementation the contract. This plan covers:

- expanded-sidebar geometry;
- the user-owned Pin grid;
- the app-owned runtime activator area;
- a breaking redesign of the in-development JavaScript shell API;
- shared persistence and platform render contracts;
- implementation commits and live Windows verification.

The shared behavior described here is authoritative. macOS is a useful visual
reference for a narrow sidebar and native text/scroll behavior; Windows already
has useful active-state and compact-overflow behavior. The result intentionally
combines those strengths instead of copying either implementation wholesale.

## Architecture boundary

- `lingxia-shell` is the platform-neutral semantic owner. It contains typed
  activator/Pin state, validation, versioned stores, declaration generations,
  stable-id routing, and the combined eight-Pin limit.
- `lingxia-surface` remains the generic presentation graph. It knows main,
  aside, slot, focus, visibility, and layout plans; it does not know Pins,
  bookmarks, activators, or terminal product behavior.
- The top-level `lingxia` crate coordinates the two domains. A shell activation
  intent is projected into the surface graph or a native host capability.
- Logic only parses the JS declaration and owns generation-scoped callbacks.
  Platform SDKs only render resolved snapshots and report stable ids.
- This is an in-development breaking change. There are no old-schema readers,
  compatibility adapters, or development-data migrations.

## Non-goals

- An app API for creating or closing browser tabs.
- An app API for changing user Pins.
- App control over sidebar width or collapsed state.
- Requiring runtime activators to have matching `surfaces:` entries in
  `lingxia.yaml`.
- Pixel-identical system chrome in the compact rail. macOS may remain wider to
  clear the traffic lights.

## 1. Expanded sidebar and Pin grid

### Sidebar width

- Use `184` logical units as the default expanded width on both desktop
  platforms.
- Windows reduces its current `220` minimum/default width to `184`.
- macOS increases its current `180` expanded baseline to `184`; it may remain
  user-resizable.
- The compact rail remains platform-native. Expanded content geometry, not the
  compact rail width, is the parity boundary.

### Pin geometry and capacity

- Four fixed columns.
- Two rows.
- Eight Pins maximum, counting lxapp and web Pins together.
- Tile size: `36 x 36`.
- Horizontal and vertical gap: `5`.
- Grid width: `4 * 36 + 3 * 5 = 159`.
- Center the fixed grid within the sidebar content area.
- Keep incomplete rows aligned to the first grid slot. Do not redistribute
  occupied tiles based on the current count, because that moves existing Pins
  when another Pin is added.
- The grid never scrolls. Eight is the high-frequency set; bookmarks and normal
  app/tab navigation hold the long tail.

### Pin ownership and enforcement

- Pins are user-owned shell state, not writable through the app JS API.
- Enforce the combined limit in shared Rust code, not in Windows/macOS entry
  points and not only at render time.
- Every Pin mutation path must use the shared operation: native page menu,
  address bar, context menu, bookmark manager, and lxapp Pin menu.
- Reaching the limit returns a typed `LimitReached { max: 8 }` result. Platform
  chrome shows a localized message instead of silently logging or truncating.
- Persist one ordered mixed Pin list so user order is preserved across lxapp and
  web targets. Platform renderers must not force all lxapps before all websites.
- Do not render-truncate a successfully stored Pin. Stored and visible state
  must agree.

## 2. Runtime activator model

An activator is an app-declared persistent shell entry. It either activates
dynamic content or invokes Logic. It is not synonymous with the whole sidebar
and is not a shortcut to a statically declared YAML surface.

### Relationship to `lingxia.yaml`

- YAML owns static capabilities, permissions, packaged resources, and launch
  structure.
- JS owns the runtime activator list, its order, presentation metadata, and
  action callbacks.
- An lxapp activator does not require a matching YAML surface. The target only
  needs to be resolvable from a bundled, installed, or runtime-provided lxapp.
- A native activator does not require a YAML surface entry. It does require the
  corresponding host capability; for example, terminal activation requires
  `capabilities.terminal: true`.
- An action activator has no YAML dependency.

### Activation behavior

For an lxapp target:

1. If it is already open as a main, focus it.
2. If it is already open as an aside, toggle its visibility.
3. If it is not open, resolve/load it and present it as an adaptive aside.

For a native target:

1. Verify the host capability.
2. Toggle the capability using its host-owned default presentation; terminal
   defaults to a bottom aside.

For an action target:

1. Invoke the currently registered Logic `onActivate` callback.
2. Never treat it as selected/active.

A disabled activator stays visible but cannot activate.

## 3. JavaScript API redesign

The entire JS shell API is still in development, so this is an intentional
breaking redesign. Keep the semantic namespace independent from its current
desktop placement:

```ts
lx.shell.activators.replace([
  {
    id: 'assistant',
    lxapp: 'com.example.assistant',
    label: 'Assistant',
  },
  {
    id: 'terminal',
    native: 'terminal',
    label: 'Terminal',
  },
  {
    id: 'sync',
    label: 'Sync',
    icon: 'icons/sync.svg',
    onActivate() {
      startSync();
    },
  },
]);

lx.shell.activators.update('sync', {
  label: 'Syncing…',
  disabled: true,
});

lx.shell.activators.remove('sync');
lx.shell.activators.clear();
```

### Public types

```ts
interface ShellActivatorBase {
  /** Stable activator identity used by update/remove and persistence. */
  id: string;
  label?: string;
  icon?: string;
  disabled?: boolean;
}

type ShellActivator =
  | (ShellActivatorBase & {
      lxapp: string;
      native?: never;
      onActivate?: never;
    })
  | (ShellActivatorBase & {
      native: 'terminal';
      lxapp?: never;
      onActivate?: never;
    })
  | (ShellActivatorBase & {
      label: string;
      icon: string;
      onActivate: () => void;
      lxapp?: never;
      native?: never;
    });

interface ShellActivatorUpdate {
  label?: string;
  icon?: string;
  disabled?: boolean;
}

interface ShellActivatorsApi {
  /** Atomically replace the complete declaration. */
  replace(items: ShellActivator[]): void;
  update(id: string, patch: ShellActivatorUpdate): void;
  remove(id: string): void;
  clear(): void;
}
```

### DX decisions

- Use plural `activators`; it is a collection and does not claim ownership of
  the complete sidebar.
- Use `replace`, not `set`, so full-list replacement is explicit.
- Use `label`, matching the rendered concept and internal naming.
- Use `onActivate`, not `onClick`: mouse, keyboard, accessibility, shortcut,
  and automation activation all have identical semantics.
- Every entry has an explicit stable `id`. Target values are not overloaded as
  update keys and different target kinds cannot collide accidentally.
- Remove `weight`; application code must not control platform row allocation.
- Remove arbitrary `color`; it conflicts with themes, contrast, disabled state,
  and shell-owned active styling.
- Do not expose `surface` as the target. Dynamic lxapp/native activators remain
  valid without YAML surface declarations.
- `replace` validates the complete generation before changing handlers,
  persistence, or platform chrome. A bad item leaves the old generation intact.
- Invalid arguments and unsupported declared native capabilities throw. The
  resultless cosmetic API remains a no-op on platforms without desktop shell
  chrome, following the existing `lx.*` portability contract.
- Only the home lxapp may write activators. State this on every method's JSDoc,
  not only on the parent namespace.
- Document icon path resolution, desktop availability, persistence behavior,
  activation behavior, and `replace([])` semantics in generated declarations
  and the repo skill.

`remove` and `clear` may be implemented as atomic transformations of the same
full generation. They are DX conveniences, not separate platform mutation
protocols.

## 4. Shared state, persistence, and render contract

### Rust-owned state

Move activator truth into a versioned shared Rust store instead of maintaining
separate semantic resolution in Swift and the Windows SDK:

```json
{
  "version": 1,
  "declared": true,
  "items": [
    {
      "id": "assistant",
      "target": { "kind": "lxapp", "key": "com.example.assistant" },
      "label": "Assistant",
      "icon": null,
      "disabled": false
    }
  ]
}
```

- `declared: true` distinguishes an explicit empty declaration from no writer.
- Persist lxapp/native items so they render before Logic boots.
- Do not restore action items before Logic redeclares them; their callbacks are
  process-local.
- `replace([])` persists an explicit empty generation and remains empty after
  restart.
- Restore the same surface-item generation on Windows and macOS.
- Keep action callback registration generation-scoped. Replacing or removing an
  item unregisters the previous callback.

### Resolved render model

Shared Rust code resolves target metadata and state into a platform-neutral
render list:

```ts
interface ResolvedShellActivator {
  id: string;
  kind: 'lxapp' | 'native' | 'action';
  label: string;
  iconPath?: string;
  active: boolean;
  disabled: boolean;
}
```

- Resolve fallback lxapp label/icon once in Rust.
- Derive `active` from the managed presentation graph for lxapp/native items.
- Platform renderers consume the resolved model; they do not reinterpret target
  semantics, fallback labels, or capability behavior.
- Platform code reports activation by stable `id`; shared Rust routes it to the
  target or Logic callback.

## 5. Activator visual and interaction parity

Use a shared behavioral spec rather than treating one platform implementation
as authoritative.

### Expanded footer

- Outer horizontal extent aligns with top-level sidebar rows: `8` inset.
- Cell height: `30`.
- Cell gap and row gap: `4`.
- Minimum cell width: `72`.
- Maximum visible rows: `5`; overflow scrolls inside the footer.
- Inactive background is transparent.
- Hover uses a quiet shell-owned wash with radius `6`.
- Active lxapp/native items use a light selected background plus accent marker.
- Disabled items use muted icon/text, no hover wash, and no activation.
- Action items never show active state.
- Long labels truncate at the tail and expose the complete label as tooltip and
  accessibility text.
- Each platform measures text using its native font metrics. Do not keep the
  current Windows ASCII/wide-character width heuristic.
- Row breaks may differ when native fonts genuinely differ, but padding,
  minimums, state treatment, overflow, and order remain identical.

### Compact rail

- Render icon-only activators with their label as tooltip/accessibility text.
- Reserve the expand control; activators may not overlap it or run off-window.
- Apply the same bounded scrolling behavior on both platforms.
- Preserve active and disabled treatment in icon-only form.
- Compact rail width may remain platform-specific for system-chrome clearance.

## 6. Implementation commits

- `feat(shell): introduce typed host shell state core`
  - Adds the platform-neutral activator and Pin domain.
- `feat(shell): centralize activators and sidebar pins`
  - Integrates Logic, the top-level coordinator, both desktop renderers,
    bookmark projections, native capabilities, and transactional host apply.
- `docs(shell): document activator and Pin architecture`
  - Updates the repo skill, contract, verification record, and showcase.

The implementation intentionally has no adapter for the discarded JavaScript
API, old activator persistence schema, or old lxapp-only Pin store. This branch
is pre-merge development and starts directly on the final contract.

## 7. Verification plan

### Automated gates

- `cargo fmt --all -- --check`
- Activator API/store unit tests in `lingxia-logic` or the new shared owner.
- Browser bookmark/Pin store tests.
- Windows sidebar geometry and hit-test tests at width `184`.
- Type generation followed by the package TypeScript check.
- `cargo check --workspace --all-targets`.
- `cargo clippy -p lingxia-windows-sdk --features browser-shell --all-targets`.

The Apple implementation must at least compile in PR CI; add focused Swift
tests for the decoded resolved model and bounded layout where the package test
structure permits it.

### Completed local verification

- Rust format, workspace check, shell/browser/Logic/Windows unit tests, and
  Windows browser-shell clippy pass.
- Generated Logic declarations, generated declaration quality, and i18n output
  checks pass.
- The bookmarks manager inline script parses after moving Pin state to the
  separate `pinnedIds` projection.
- A release Windows showcase run driven entirely through `lxdev` verified the
  `184` sidebar, resolved active and disabled activators,
  replace/update/remove/clear, duplicate-id atomicity, surface-only persistence,
  and real pointer activation (`disabled: 0 -> 0`, action: `0 -> 1`).
- The same run cold-opened and toggled the Chat aside, opened and toggled the
  dynamic terminal, and captured browser main + Chat right aside + Terminal
  bottom aside simultaneously. Stable surface id `shell:terminal` remained
  separate from native content id `terminal` in the surface graph.
- Runtime logs contained no errors and no shell/browser layout warnings after
  the exercised interactions. Accessibility activation was not automated in
  the current Windows session. The macOS changes were statically reviewed
  locally and are compiled by PR CI.

### Local Windows showcase

Build fresh CLIs and refresh the PATH copies before runtime testing:

```powershell
cargo build -p lingxia-cli -p lingxia-devtools-cli
Copy-Item target/debug/lingxia.exe ~/.local/bin/lingxia.exe -Force
Copy-Item target/debug/lxdev.exe ~/.local/bin/lxdev.exe -Force
```

From `examples/lingxia-showcase`:

```powershell
npm install
lingxia dev -p windows --release --framework react --background
```

Exercise and record all of the following with `lxdev`:

1. Expanded sidebar width and a full `4 x 2` Pin grid.
2. Ninth Pin attempt is rejected with localized feedback and no hidden stored
   Pin.
3. Mixed lxapp/web Pin order survives a host restart.
4. `replace` displays dynamic lxapp, native terminal, and action activators
   without matching YAML surfaces.
5. Dynamic lxapp activation cold-opens as aside, toggles while aside, and
   focuses if already main.
6. Native terminal activation uses the default bottom aside.
7. Mouse and keyboard/accessibility activation both invoke `onActivate` once.
8. `update`, `remove`, `clear`, disabled state, long-label truncation, expanded
   overflow, and compact overflow.
9. Persisted lxapp/native entries appear before Logic declaration after restart;
   action entries do not appear until handlers are redeclared.
10. `replace([])` remains empty across restart.
11. Full-app screenshots for expanded, active, disabled, overflow, and compact
    states.
12. `lxdev logs` contains no new native, Logic, or browser errors after each
    interaction set.

Stop the dev session after verification.

## 8. PR handoff

After all gates and live checks pass:

- push the scoped commits to `feat/shell-ui-spec`;
- update PR #126 with the final contract, breaking-API notes, commit breakdown,
  exact commands run, live observations, and screenshot references;
- explicitly call out any macOS behavior verified only by CI rather than live;
- do not include unrelated worktree files such as scratch images.
