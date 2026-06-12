# lingxia-windows

Pure Rust Windows host SDK for LingXia.

`lingxia-windows` is the crate a native Windows executable uses to boot the
LingXia runtime, open the home lxapp, register Windows host components, and
run the Win32 message loop.

It is intentionally not the Windows runner application and not the LingXia
shell product UI.

## Scope

This crate owns Windows host functionality that should be available to any
Rust Windows host:

- runtime bootstrap through `WindowsApp`, `init`, `run_message_loop`, and
  `quick_start`
- app identity, asset, cache, data, locale, icon, and initial window-size
  configuration
- registration of embedded native components such as text input and video
  overlays
- optional app-window device frame and app menu hooks exposed to host
  executables

The crate delegates generic WebView2 windowing to `lingxia-webview`, platform
traits to `lingxia-platform`, and public runtime facade calls to `lingxia`.

## Boundaries

Keep these responsibilities out of `lingxia-windows`:

- runner application appearance and simulator/device chrome; that belongs in
  `tools/lingxia-runner/windows`
- LingXia shell product chrome such as sidebar layout, address bar behavior,
  browser tab rows, downloads/settings actions, and panel activators; that
  belongs in `lingxia-shell`
- generic WebView2 mechanics such as window creation, WebView controller
  ownership, resize/layout plumbing, events, schemes, and menus; that belongs
  in `lingxia-webview`
- cross-platform public facade APIs; those belong in `lingxia`

If a feature is needed by every Windows host regardless of product shell, it is
a candidate for this crate. If it is a visual/product policy decision, keep it
in the runner or shell layer.

## Minimal Host

```rust
fn main() -> lingxia_windows::Result<i32> {
    lingxia_windows::quick_start()
}
```

For custom paths and identity:

```rust
fn main() -> lingxia_windows::Result<i32> {
    let app = lingxia_windows::WindowsApp::new("data", "cache", "assets")
        .with_app_identifier("com.example.app")
        .with_product_name("Example")
        .with_window_size(1200, 800);

    lingxia_windows::init(app)?;
    Ok(lingxia_windows::run_message_loop())
}
```

`init` and `run_message_loop` must run on the same thread.
