# LingXia Facade Boundary

> **Audience**: contributors working on the `lingxia` crate facade and the
> implementation crates behind it. App authors only need "import from
> `lingxia::*`, never from internal crates", stated in
> [`docs/skill/native/development.md`](../skill/native/development.md). Which
> crate owns which code is [`crate-layout.md`](crate-layout.md).

`lingxia` is the only public Rust API for LingXia app authors. Invariants:

- An app's Rust library depends on `lingxia` and nothing else from LingXia.
  A host app may also link the host integration crates
  (`lingxia-control-runtime`, `lingxia-windows-sdk`).
- The public API is organized around what a developer means (host functions,
  files, update, media), not around internal crate layout.
- An app crate that needs an internal crate means the facade is missing an
  export or wrapper; add it to the facade rather than taking the dependency.

The reason is that internal crates are reorganized freely. Only the facade has
to stay stable, so an app crate that reaches past it breaks on refactors that
never touched its API.

## What the facade exports

| Area | Exports | Notes |
| --- | --- | --- |
| Platform entry | `lingxia::android`, `apple`, `harmony`, `windows`; the init path | The facade owns the final FFI surface. Apps never call `lxapp::init` or wire runtime internals. |
| Host functions | `#[lingxia::native(...)]`, `lingxia::host::*` (`HostResult`, `HostCancel`, `await_or_cancel`, `StreamContext`, `ChannelContext`), `HostAddon`, `register_host_addon` | The proc-macro crate is an implementation detail. |
| Logic extensions | `lingxia::js::{LxLogicExtension, register_logic_extension}` (`standard` feature) | Apps never depend on `lxapp::lx`. |
| Runtime types | `LxApp`, surface and page request types, provider traits (`lingxia::provider`) | Only types an author legitimately needs; no runtime orchestration types. |
| Product domains | `app`, `device`, `file`, `media`, `update`, `task`, `wifi`, `location`, `network`, `navigation`, `log` | Grouped by what they do, not by owning crate. |
| Opt-in products | `embedded_browser` (`browser-runtime`), `terminal` (`terminal-runtime`), `capture`, `automation_runtime`, `product_cli`, `dev` (`devtool`) | Each behind its feature, so a product build without it carries none of the code. |

## Host app update

The surface is `lingxia::update::host_app`, a complete check + install flow
rather than raw package data:

```rust
// Optional, once at startup; without this the platform default installer is used.
lingxia::update::host_app::set_installer(|package_path| my_install(package_path));

// Optional, single-slot progress handler for native UI.
lingxia::update::host_app::on_progress(|progress| {
    // Render native UI from `Progress` events.
});

// Runs check + download + install hand-off in one call.
match lingxia::update::host_app::check().await? {
    lingxia::update::host_app::Outcome::UpToDate => {}
    lingxia::update::host_app::Outcome::Installed { version } => {}
    lingxia::update::host_app::Outcome::Deferred { version } => {}
}
```

The facade exposes no package paths, caller-supplied current versions, global
config mutation, or separate download/install steps, so a host cannot install
a package that skipped verification or the version check (see
[update-pipeline.md](update-pipeline.md)).

## Downloads and the embedded browser

Download history, pause, retry and directory overrides are shell and Logic
product concerns. They stay in `lingxia-service` / `lingxia-transfer` and are
not facade API. Native code that just needs a file uses
`lingxia::file::download`.

A host embedding its own browser controls uses `lingxia::embedded_browser`.
It configures the transport (`require_local_proxy`, `reapply_local_proxy`,
`block_downloads`, `use_system_network`) and reads browser-owned download
state (`downloads`, `download_action`), never with caller-supplied filesystem
paths. The proxy contract (fail-closed, per-platform scope, threading) is
documented on the module's rustdoc in `crates/lingxia/src/embedded_browser.rs`;
keep it there.

## What stays internal

| Internal | Why |
| --- | --- |
| `lxapp::init`, install/load orchestration, built-in app ownership, route wiring | Sequenced by the facade's init path; apps have no reason to call them. |
| Page attach/detach, popup and overlay orchestration, page target resolution, WebView controller lifecycle | Platform lifecycle with no authoring use; see [webview-lifecycle.md](webview-lifecycle.md). |
| Browser tab models and `lingxia-browser` | The browser product registers itself at init when its feature is on; apps reach it only through `embedded_browser`. |
| `lingxia-native-macros` | Re-exported as `lingxia::native`. |
| First-party routes such as `lingxia://settings` | Apps open product surfaces through narrow typed APIs (`RuntimeInfo::resolve_settings_destination`, `RuntimeInfo::open_trusted_browser_page`), never raw routes. |

## Windows host entry

`lingxia-windows-sdk` is the Windows host entry: the binary-level pieces
(`WindowsApp`, `init_runtime`, `run_message_loop`, `quick_start`).
`lingxia::windows` holds the facade-level Windows APIs it builds on
(`Platform`, `init`, `open_home_app`). Hosts use `lingxia-windows-sdk` for the
binary entry and the message loop.
