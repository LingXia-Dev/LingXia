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

- runtime bootstrap through `WindowsApp`, `init`, `run_message_loop`, and
  `quick_start`
- generated `app.json` host identity loading (`productName`, `windowsAppId`),
  plus derived state directories, locale, bundled icon, and initial
  window-size configuration
- registration of embedded native components such as text input and video
  overlays
- optional default shell UI: window chrome, sidebar/tabbar layout, address bar
  behavior, and panel activators
- optional app-window device frame and app menu hooks exposed to host
  executables

The crate delegates generic WebView2 windowing to `lingxia-webview`, platform
traits to `lingxia-platform`, and public runtime facade calls to `lingxia`.

## Boundaries

Keep these responsibilities out of `lingxia-windows-sdk`:

- runner application appearance and simulator/device chrome; that belongs in
  `tools/lingxia-runner/windows`
- generic WebView2 mechanics such as window creation, WebView controller
  ownership, resize/layout plumbing, events, schemes, and menus; that belongs
  in `lingxia-webview`
- cross-platform public facade APIs; those belong in `lingxia`
- app identity and bundle metadata decisions; those belong in `lingxia.toml`
  and the generated `assets/app.json`

If a feature is needed by every Windows host regardless of product shell, it is
a candidate for this crate. If it is a visual/product policy decision, keep it
in the runner or shell layer.

## Minimal Host

```rust
fn main() -> lingxia_windows_sdk::Result<i32> {
    lingxia_windows_sdk::quick_start()
}
```

`quick_start` reads the generated `assets/app.json` when present, matching the
Apple SDK pattern where host metadata comes from bundled config rather than
from the application entry point.

`init` and `run_message_loop` must run on the same thread.
