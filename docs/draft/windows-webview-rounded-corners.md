# Windows content WebView2 — rounded corners (composition hosting)

Status: design / not yet implemented. Branch: `feature/windows-webview-radius`.

## Goal

Give the main content WebView2 surface anti-aliased rounded corners matching the
shell content card (`SHELL_PANEL_RADIUS = 14px`), the way macOS rounds its content
view. The shell already paints an AA rounded card behind the webview
(`draw_content_card` → `fill_round_rect_aa`), but the webview is a rectangular
child window that overpaints the card's corners.

## Why composition (and not the alternatives)

The webview is hosted in **windowed mode** today:
`env.CreateCoreWebView2Controller(hwnd)` (`crates/lingxia-webview/src/windows/environment/settings.rs`),
positioned with `controller.SetBounds(...)` (`controller.rs`).

- `SetWindowRgn` + `CreateRoundRectRgn` — works, but GDI regions are **not
  anti-aliased** → jagged corners that clash with the AA card. Rejected.
- DWM `DWMWA_WINDOW_CORNER_PREFERENCE` — only applies to **top-level** windows,
  not the child webview. Not applicable.
- **Composition hosting** (`CreateCoreWebView2CompositionController` + a WinRT
  `Compositor` visual tree) — supports an **AA `CompositionGeometricClip`**.
  This is the only clean option, and is also the more future-proof WebView2
  hosting model on Windows.

## Architecture

Render the webview into a WinRT composition visual tree hosted on the window:

```
DesktopWindowTarget (bound to host HWND)
  └─ root ContainerVisual               clip = CompositionGeometricClip(
       └─ webview RootVisualTarget visual    CompositionRoundedRectangleGeometry,
                                              corner radius = SHELL_PANEL_RADIUS)
```

Input is **not** delivered automatically in composition hosting — the host window
proc must forward it.

## Implementation steps

Order matters; each builds on the previous. The repo has **no existing WinRT
composition usage**, so the bootstrapping (steps 1–2) is new.

1. **Cargo features** (`crates/lingxia-webview/Cargo.toml`, `windows` deps):
   add `Foundation`, `Foundation_Numerics`, `System`, `UI_Composition`,
   `UI_Composition_Desktop`, `Win32_System_WinRT`,
   `Win32_System_WinRT_Composition`.

2. **Thread bootstrap (UI thread, once)**: a `Compositor` requires a
   `DispatcherQueue` on the thread. On the window's UI thread call
   `CreateDispatcherQueueController` (`DispatcherQueueOptions{ DQTYPE_THREAD_CURRENT,
   DQTAT_COM_NONE }`) and keep the controller alive; ensure the thread is WinRT-
   initialized (`RoInitialize`, STA/MTA consistent with the existing COM init).

3. **Compositor + target**: `let compositor = Compositor::new()?;`
   `let target = compositor.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(hwnd, false)?;`
   `let root = compositor.CreateContainerVisual()?;` `target.SetRoot(&root)?;`
   Size `root` to the content rect; size with the webview.

4. **Composition controller**: replace `create_controller` with
   `env.CreateCoreWebView2CompositionController(hwnd, handler)` →
   `ICoreWebView2CompositionController`. Get the base `ICoreWebView2Controller`
   via `.cast()` for the existing settings/visibility/close paths.
   Set `composition_controller.SetRootVisualTarget(&webview_visual)` where
   `webview_visual` is a child `SpriteVisual`/`ContainerVisual` of `root`.

5. **Rounded clip (the actual feature)**: on `root` (or the webview visual):
   ```
   let geo = compositor.CreateRoundedRectangleGeometry()?;
   geo.SetCornerRadius(Vector2{ x: RADIUS, y: RADIUS })?;
   geo.SetSize(content_size)?;            // update on resize
   let clip = compositor.CreateGeometricClipWithGeometry(&geo)?;
   root.SetClip(&clip)?;                   // SetClip takes CompositionClip
   ```

6. **Bounds / resize / DPI**: composition controller still uses
   `put_Bounds` for the webview pixel size; the visual `Offset`/`Size` and the
   clip geometry size must track the content rect (hook the existing
   `set_content_bounds` path in `controller.rs`). Account for DPI scale.

7. **Input forwarding (highest risk)**: composition-hosted WebView2 receives no
   mouse/pointer input. In the host window proc
   (`crates/lingxia-windows-sdk/src/window_host.rs`), when the cursor is over the
   webview content rect, forward `WM_MOUSE*` / `WM_POINTER*` / `WM_MOUSEWHEEL`
   via `ICoreWebView2CompositionController::SendMouseInput` / `SendPointerInput`
   (map to `COREWEBVIEW2_MOUSE_EVENT_KIND`, virtual keys for modifiers, wheel
   delta; coordinates relative to the webview). Handle `WM_SETCURSOR` using the
   controller's cursor (`get_Cursor` / `CursorChanged`). Keyboard/focus still
   flow via the parent HWND + `MoveFocus`.

## Verification (must run locally)

Composition + input can only be proven by running — there is **no incremental
milestone that proves "clicks work" without the input forwarding** (step 7), so
verify as one gate after steps 1–7:

- WebView content renders (not black) and resizes with the window.
- Corners are rounded and **anti-aliased**, flush with the shell card.
- Mouse: click, hover, drag, text selection, scroll wheel all work in the page.
- Keyboard + focus work (type into an input, Tab navigation).
- DPI: correct on 100% / 150% / 200%.
- No regression to existing webview features (devtools, navigation, panels).

## Risk / rollback

This replaces the core webview hosting model. A subtle input-forwarding bug makes
the app unclickable. Keep the change behind the branch until fully verified;
consider a feature flag to fall back to windowed hosting if composition setup
fails at runtime (e.g. `CreateDispatcherQueueController` / compositor errors →
log + use windowed `CreateCoreWebView2Controller`, square corners).

## Files

- `crates/lingxia-webview/Cargo.toml` — windows composition features.
- `crates/lingxia-webview/src/windows/environment/settings.rs` — controller
  creation (composition).
- `crates/lingxia-webview/src/windows/controller.rs` — controller struct,
  bounds, visual tree, clip, close.
- `crates/lingxia-windows-sdk/src/window_host.rs` — mouse/pointer input
  forwarding + cursor.
- (radius source) `crates/lingxia-windows-sdk/src/shell/style.rs`
  `SHELL_PANEL_RADIUS`.
