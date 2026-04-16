# LingXia Facade Boundary

This internal document defines the intended public Rust API boundary for LingXia application projects.

The target state is simple:

- app-facing Rust libraries depend on `lingxia`
- app-facing Rust libraries do not depend directly on `lxapp`
- internal implementation layers such as `lingxia-browser`, the shell domain inside `lingxia`, and proc-macro crates stay behind the `lingxia` facade

## Goal

`lingxia` is the only public Rust facade for LingXia app authors.

Application Rust libraries, such as `examples/*/rust-lib`, should:

- depend on `lingxia`
- define host APIs through `lingxia`
- register logic extensions through `lingxia`
- consume platform exports through `lingxia`

They should not need to know about runtime-core crate boundaries.

## What `lingxia` Must Export

### 1. Platform entry surface

`lingxia` should remain the crate that produces and owns the final platform-facing entry surface.

It should export:

- platform FFI modules such as `lingxia::android`, `lingxia::apple`, and `lingxia::harmony`
- the unified initialization path used by host apps
- built-in product registration needed for browser, settings, downloads, and other first-party surfaces

Application crates should not call `lxapp::init` or wire runtime internals directly.

### 2. Host API authoring surface

App authors should define page-facing host functions only through `lingxia`.

This includes:

- `#[lingxia::native(...)]`
- `lingxia::host::*`
- `lingxia::host::HostResult`
- `lingxia::host::HostCancel`
- `lingxia::host::await_or_cancel(...)`
- `lingxia::host::StreamContext`
- `lingxia::host::ChannelContext`

The proc-macro implementation can stay in a separate crate, but that crate is an implementation detail.

### 3. Logic extension authoring surface

App authors should register Logic-layer APIs through `lingxia`.

This includes:

- `lingxia::LxLogicExtension`
- `lingxia::register_logic_extension(...)`
- any stable `lx` helper facade intentionally exposed for extension authors

App crates should not depend on `lxapp::lx` directly.

### 4. Stable runtime context types

`lingxia` should export the minimum set of runtime types an app author legitimately needs.

Today that includes types such as:

- `lingxia::LxApp`
- provider traits intended for app integration
- stable download and update surface types

The rule is:

- export app-authoring types
- do not export internal runtime orchestration types unless there is a clear authoring need

### 5. Product-domain facades

If an app crate can reasonably use a capability, it should come from `lingxia`, not from an internal crate.

Examples:

- downloads
- app info
- preferences/settings
- update providers
- notification providers
- fingerprint providers

These should be organized around product domains, not around internal crate ownership.

## What `lingxia` May Export Carefully

### 1. Browser product facade

If app authors need to enable or register the built-in browser product, that should happen through `lingxia`.

Examples of acceptable facade shapes:

- `lingxia::register_builtin_browser()`
- `lingxia::browser::register_builtin()`

App crates should not need a direct dependency on `lingxia-browser`.

### 2. Settings and internal product actions

If app authors need to open first-party product surfaces such as Settings or Downloads, that should also go through `lingxia`.

Examples:

- `lingxia::open_settings(...)`
- `lingxia::open_downloads(...)`

App code should not need to know internal routes such as `lingxia://settings`.

### 3. Preferences facade

If app authors need direct configuration access, expose a stable preferences/settings facade from `lingxia`.

Do not make app crates reach into shell or browser implementation modules.

## What Must Stay Internal

### 1. `lxapp` initialization and runtime wiring

The following belong to internal implementation layers:

- `lxapp::init`
- runtime install/load orchestration
- built-in app ownership resolution
- internal route wiring

These are not part of the application authoring surface.

### 2. WebView and page lifecycle internals

Do not expose implementation details such as:

- page attach and detach internals
- popup and overlay orchestration
- browser tab runtime models
- internal page target resolution
- WebView controller lifecycle details

### 3. Browser and shell implementation crates

Application crates should not directly depend on:

- `lingxia-browser`

Those crates can exist internally during transition, but they should not define the public authoring model.

### 4. Macro implementation crates

Application crates should not directly depend on:

- `lingxia-macro`
- `lingxia-host-macros`

These are implementation details behind `lingxia::native`.

## Recommended Public Shape

The long-term public API should look more like this:

```rust
lingxia::
  android
  apple
  harmony

  host
  native

  LxApp
  LxLogicExtension
  register_logic_extension

  downloads
  updates
  preferences
  browser
  settings
```

The important rule is:

- public API should be organized around developer meaning
- public API should not mirror internal crate layout

## Rules For App Rust Libraries

Application Rust libraries may depend on:

- `lingxia`

Application Rust libraries should not directly depend on:

- `lxapp`
- `lingxia-browser`
- `lingxia-macro`

If an app crate currently needs one of those dependencies, that indicates the `lingxia` facade is still missing a required export or wrapper.

## Execution Plan

To move the workspace to this model:

1. Keep this document as the contract for public Rust API cleanup.
2. Inventory every app/example crate that depends directly on `lxapp`, `lingxia-browser`, or macro crates.
3. For each direct dependency, identify what public capability is missing from `lingxia`.
4. Add the missing re-exports or facade APIs to `lingxia`.
5. Remove the direct internal-crate dependency from the app crate.
6. Repeat until all target app Rust libraries depend only on `lingxia`.

## Current Direction

The current architectural direction should be:

- `lingxia` is the public facade and final product entry crate
- `lxapp` is runtime core
- browser and shell concerns stay behind `lingxia`
- app authors only program against `lingxia`

That is the boundary future refactors should preserve.
