# lingxia-windows-sdk

Pure Rust Windows host SDK for LingXia.

`lingxia-windows-sdk` is the crate a native Windows executable uses to boot the
LingXia runtime, open the home lxapp, register Windows host components, and
run the Win32 message loop.

It is intentionally not the Windows runner application and not the LingXia
shell product UI.

## Scope

This crate owns Windows host functionality that should be available to any
Rust Windows host:

- runtime bootstrap through `WindowsApp`, `init_runtime`, `run_message_loop`,
  and `quick_start`
- generated `app.json` host identity loading (`productName`, `windowsAppId`),
  plus derived state directories, locale, bundled icon, and initial
  window-size configuration
- registration of embedded native components such as text input and video
  overlays
- optional default shell UI: window chrome, sidebar/tabbar layout, address bar
  behavior, and panel activators
- optional app-window device frame and app menu hooks exposed to host
  executables

The crate delegates generic WebView2 controller mechanics to
`lingxia-webview`, platform traits to `lingxia-platform`, and public runtime
facade calls to `lingxia`.

## Boundaries

Keep these responsibilities out of `lingxia-windows-sdk`:

- runner application appearance and simulator/device chrome; that belongs in
  `tools/lingxia-runner/windows`
- generic WebView2 mechanics such as environment setup, controller ownership,
  navigation, events, scheme plumbing, and binding a controller to a supplied
  parent HWND; that belongs in `lingxia-webview`
- product-specific host window policy and layout beyond the reusable SDK shell;
  keep those in the target app or runner layer
- cross-platform public facade APIs; those belong in `lingxia`
- app identity and bundle metadata decisions; those belong in `lingxia.toml`
  and the generated `assets/app.json`

If a feature is needed by every Windows host regardless of product shell, it is
a candidate for this crate. If it is a visual/product policy decision, keep it
in the runner or shell layer.

## Modes

Two usage modes, selected by Cargo features:

- **quick-start** (`standard` — the default, or `browser-shell` for the native
  shell): batteries included. `quick_start` boots the runtime and pumps the
  Win32 message loop until the app exits.
- **advanced**: the host brings its own window and message loop and registers
  its own `WindowsHostBackend` (from `lingxia-windows-contract`). It boots the
  runtime with the host-agnostic `init_runtime` (which presents no window),
  optionally enabling the `components` tier and calling
  `install_windows_components` for the SDK's view overlays. See
  `examples/advanced_host.rs`.

Feature tiers: `host-api` ⊂ `components` ⊂ `runtime` ⊂ `standard`/`browser-shell`.

Boot API:

- `init_runtime(app) -> home_app_id` — host-agnostic: boots the runtime,
  presents no window, installs no backend.
- `install_windows_components()` — installs SDK-managed native component
  integrations without installing the default backend.
- `install_default_windows_host()` — installs the SDK's default WebView
  parent-window host + backend + components + app menu (+ shell under
  `browser-shell`).
- `start_default_host(app) -> home_app_id` — `install_default_windows_host` +
  `init_runtime` + opens the home window, *without* pumping the loop (for hosts
  that want the default UI but their own post-boot setup, e.g. the runner).
- `quick_start()` — `start_default_host` + `run_message_loop`.

## Minimal Host (quick-start)

```rust
fn main() -> lingxia_windows_sdk::Result<i32> {
    lingxia_windows_sdk::quick_start()
}
```

`quick_start` reads the generated `assets/app.json` when present, matching the
Apple SDK pattern where host metadata comes from bundled config rather than
from the application entry point.

`init_runtime` and `run_message_loop` must run on the same thread.
