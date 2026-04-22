# LingXia Shell Product Abstraction Draft

`lingxia-shell` is a good product-level abstraction, but its current shape is
too close to a collection of shell implementation details. The goal is to make
it represent LingXia Shell as a product capability layer, not a bundle of
platform-specific implementation modules.

## Problem

The current shell crate mixes several concerns:

- product-level shell surfaces such as browser, downloads, settings, and panels
- host routes and bridge adapters
- shell webui asset registration
- platform fallback behavior such as open/reveal
- optional cross-platform proxy behavior
- desktop/macOS-specific chrome behavior

This makes `lingxia-shell` feel like a bundle of implementations rather than a
clear product abstraction. It also makes `lingxia` and the CLI reason about
shell internals instead of product-level capabilities.

## Target Boundary

`lingxia-shell` should express product shell capabilities:

- browser surface
- downloads surface
- settings surface
- panels / assistant panel surface
- navigation chrome
- shell webui resource
- shell host routes

It should not expose implementation details such as platform fallbacks, WebView
quirks, proxy runtime internals, or asset-copy mechanics as its top-level model.

## Proposed Internal Shape

Keep the crate name `lingxia-shell`, but organize internals around product
concepts and implementation layers.

```text
crates/lingxia-shell
  src/
    lib.rs              // product shell public entry
    product.rs          // ShellProduct / ShellConfig / ShellCapabilities
    routes.rs           // product-level host route registration
    webui.rs            // shell webui resource contract

    surfaces/
      browser.rs        // browser shell surface contract
      downloads.rs      // downloads shell surface contract
      settings.rs       // settings shell surface contract
      panels.rs         // panel surface contract
      proxy.rs          // optional cross-platform proxy surface contract

    services/
      browser.rs        // orchestration, calls lingxia-browser/platform
      downloads.rs      // orchestration, calls lingxia-service
      settings.rs       // orchestration, calls lingxia-service
      proxy.rs          // cross-platform proxy service orchestration

    platform/
      mod.rs
      proxy.rs          // per-platform proxy integration adapters
      desktop.rs        // desktop fallback/open/reveal behavior
      macos.rs          // macOS-specific shell chrome behavior
```

The important distinction:

- `surfaces/*` describes product shell surfaces.
- `services/*` contains reusable route-facing orchestration.
- `platform/*` contains platform-specific mechanics.
- `routes.rs` adapts bridge input/output to shell services.

## Product-Level API

Top-level APIs should use product language instead of implementation language.

Possible shape:

```rust
pub struct ShellProduct {
    pub browser: bool,
    pub downloads: bool,
    pub settings: bool,
    pub panels: bool,
    pub proxy: bool,
}

pub struct ShellCapabilities {
    pub browser: bool,
    pub downloads: bool,
    pub settings: bool,
    pub panels: bool,
    pub proxy: bool,
}

pub fn install_default();
pub fn install(product: ShellProduct);
pub fn register_host_routes(product: &ShellProduct);
pub fn register_webui_bundle(source: ShellWebUiSource);
pub fn capabilities(product: &ShellProduct) -> ShellCapabilities;
```

`lingxia` should only call product-level APIs, for example:

```rust
lingxia_shell::install_default();
lingxia_shell::register_webui_bundle(...);
lingxia_shell::capabilities(...);
```

It should not need to know which concrete modules implement browser, downloads,
settings, proxy, or panels.

## Proxy Boundary

Proxy is a cross-platform optional Shell product capability. It is not macOS
only, and it should not be modeled as a desktop implementation detail.

Model it explicitly:

- Expose proxy through `ShellProduct` / `ShellCapabilities`.
- Keep route-facing orchestration under `services/proxy`.
- Keep per-platform enable/apply/clear integration under `platform/proxy`.
- Gate proxy behind a feature such as `proxy`, or keep it disabled unless the
  product config enables it.
- Do not tie proxy to macOS chrome or desktop-only behavior.

Do not leave proxy as an unqualified top-level shell concept.

## Shell WebUI Boundary

Shell webui is a product shell UI asset. It is not the host app home app, and it
is not ordinary business lxapp content.

Recommended contract:

```rust
pub struct ShellWebUi {
    pub app_id: &'static str,
    pub bundle_name: &'static str,
}

pub enum ShellWebUiSource {
    BuiltIn,
    Path(std::path::PathBuf),
    Package { name: String, version: String },
}

pub fn register_shell_webui(source: ShellWebUiSource);
```

CLI config can still expose:

```yaml
shell:
  webui:
    path: ../crates/lingxia-shell/webui
    # or:
    # package: '@lingxia/shell-webui'
    # version: '0.5.1'
```

But internally this should map to a shell webui concept, not just a normal
`resources.bundles` entry.

## Route Boundary

Shell host routes should be route adapters, not business implementations.

```text
bridge input/output
      ↓
shell route adapter
      ↓
shell service orchestration
      ↓
lingxia-service / lingxia-browser / platform
```

Example layout:

```rust
pub fn register_shell_routes(product: &ShellProduct) {
    if product.downloads {
        surfaces::downloads::register_routes();
    }
    if product.settings {
        surfaces::settings::register_routes();
    }
    if product.browser {
        surfaces::browser::register_routes();
    }
    if product.panels {
        surfaces::panels::register_routes();
    }
    if product.proxy {
        surfaces::proxy::register_routes();
    }
}
```

Route modules should parse bridge input and serialize bridge output. Shared
behavior should live in `services/*` or `lingxia-service`.

## CLI And Docs Language

CLI and docs should consistently use these concepts:

- shell product
- shell surface
- shell webui
- shell capability

Avoid describing shell as a macOS window/sidebar/settings implementation. macOS
UI is one platform presentation of the shell product, not the product model
itself.

## Migration Plan

1. Reorganize `lingxia-shell` internals without changing behavior.
2. Add `ShellProduct` and `ShellCapabilities`.
3. Make `lingxia` call only product-level shell APIs.
4. Move downloads/settings route logic into route adapters plus `services/*`.
5. Move proxy into optional cross-platform `surfaces/proxy`, `services/proxy`,
   and `platform/proxy` layers.
6. Make shell webui registration a first-class shell concept.
7. Update CLI/docs to use product language consistently.

## Non-Goals

- Do not split shell into many crates immediately.
- Do not move platform-specific code into `lingxia` just to hide it.
- Do not make shell webui the host home app.
- Do not make `resources.bundles` the only conceptual model for shell webui.
