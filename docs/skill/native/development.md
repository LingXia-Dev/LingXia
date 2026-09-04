# Native Development Guide

This guide covers the Rust native surface for LingXia host apps.

Use this guide when you want to:

- expose Rust host APIs to pages with `#[lingxia::native]`
- add optional JS AppService extensions under `lingxia::js`
- call shared LingXia SDK services from Rust through facade modules such as
  `lingxia::app`, `lingxia::task`, `lingxia::file`, `lingxia::media`, and
  `lingxia::update`

For lxapp page development, see [LxApp Development Guide](../lxapp/guide.md).
For host project configuration, see [App Project](../app/project.md).

## Host Addon

Every native host library registers a `HostAddon` before runtime initialization.
The addon is the place to declare product CLI commands, install native routes,
add optional JS extensions, and start background services.

```rust
struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    #[cfg(feature = "control")]
    fn install_product_cli(&self, cli: &mut lingxia::product_cli::ProductCli) {
        cli.command("workspace", "Manage workspaces", workspace_cli);
    }

    fn install_host_apis(&self) {
        // For each #[lingxia::native] fn, call the macro-generated companion
        // `<fn>_host()` and pass it to register_host_entry. See "The
        // macro-generated <fn>_host() companion" below.
        //
        // lingxia::host::register_host_entry(pick_document_host());
    }

    #[cfg(feature = "standard")]
    fn install_logic_extensions(&self) {
        lingxia::js::register_logic_extension(Box::new(WorkspaceDocsExtension));
    }

    fn start_services(&self) {
        #[cfg(feature = "devtools")]
        lingxia_control_runtime::start_dev_session_bridge_from_env();
    }
}

fn register_host_addon() {
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| lingxia::register_host_addon(Box::new(AppHostAddon)));
}
```

`install_product_cli` is the only pre-runtime command-registration hook. Put
the matching local-control request handler in `install_host_apis`; put neither
half in `start_services`. See [Driving a Shipped Product](../app/agent-control.md)
for the command and transport contract.

Platform entrypoints call that registration function:

```rust
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_app_MainActivity_nativeRegisterHostAddon(
    _env: jni::EnvUnowned,
    _class: jni::objects::JClass,
) {
    register_host_addon();
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    register_host_addon();
}

#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_register_host_addon() {
    register_host_addon();
}
```

Generated host templates already contain this wiring.

Besides the hooks above, an addon can also swap the launch cover per cold
start — see [Launch cover](./splash.md).

## Native Routes

Native routes expose Rust functions to the View layer. Define them with
`#[lingxia::native("namespace.method")]` and return `lingxia::Result<T>`.

```rust
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct PickDocumentInput {
    title: String,
}

#[lingxia::native("editor.pickDocument")]
async fn pick_document(
    app: Arc<lingxia::LxApp>,
    input: PickDocumentInput,
) -> lingxia::Result<String> {
    Ok(lingxia::app::state_file_for(&app, &format!("{}.md", input.title))?
        .to_string_lossy()
        .into_owned())
}
```

Supported parameters:

- optional first authority parameter: `Arc<lingxia::LxApp>` for compatibility,
  or `lingxia::host::HostInvocationContext` when the handler must authorize an
  app-owned or native-granted resource
- optional JSON payload parameter
- optional last parameter: `lingxia::host::HostCancel`

Rules:

- The authority parameter must be first when present.
- `HostCancel` must be last when present.
- Only one JSON payload parameter is supported.
- Payload types must implement `serde::Deserialize`.
- Return values must implement `serde::Serialize`.
- Handler errors should use `lingxia::Result`.

`HostInvocationContext` is created by native dispatch and cannot be constructed
from request JSON. For an authenticated lxapp caller, `app_scope()` exposes its
native identity, storage namespace, and native-issued resource grants. Treat a
payload app id or resource id only as a selector and authorize it against this
scope before access:

```rust
#[lingxia::native("editor.openGrantedDocument")]
async fn open_granted_document(
    invocation: lingxia::host::HostInvocationContext,
    resource: String,
) -> lingxia::Result<String> {
    let scope = invocation
        .app_scope()
        .ok_or_else(|| lingxia::Error::permission_denied("lxapp scope required"))?;
    let path = scope.resolve_accessible_path(&resource)?;
    Ok(path.to_string_lossy().into_owned())
}
```

Streams and channels accept the same optional first authority parameter before
their payload and final `StreamContext` or `ChannelContext`.

### Route audience metadata

`#[lingxia::native]` accepts optional registration metadata that describes the
caller class intended for a route. Ordinary host-defined routes may omit it;
the macro then records `AppSessionOnly` at compile time:

```rust
#[lingxia::native("editor.loadDocument")]
async fn load_document() -> lingxia::Result<()> {
    Ok(())
}

#[lingxia::native("host.setAccount", audience = "control-app-only")]
fn set_account() -> lingxia::Result<()> {
    Ok(())
}

#[lingxia::native("host.watch", stream, audience = "control-only")]
async fn watch_host(
    mut stream: lingxia::host::StreamContext<lingxia::host::JsonValue>,
) -> lingxia::Result<()> {
    stream.end(())?;
    Ok(())
}
```

The accepted string values are fixed by the SDK:

| String | Registration metadata |
| --- | --- |
| `app-session-only` | `AppSessionOnly` (the default for `native`) |
| `authenticated-read-only` | `AuthenticatedReadOnly` |
| `control-app-only` | `ControlAppOnly` |
| `browser-control-only` | `BrowserControlOnly` |
| `control-only` | `ControlOnly` |

An unknown value, a non-string value, or duplicate `audience` metadata is a
compile error. This metadata is fixed in the generated registration companion;
it is not a client-provided parameter and is not emitted into the generated
TypeScript or browser-global client.

`#[lingxia::framework_native(...)]` is a doc-hidden framework macro for
framework-owned routes. It shares the same syntax but requires an explicit
`audience`; application and extension authors should use `native` instead.

> **Current stage:** audience is registration metadata only. This change alone
> does not make bridge dispatch enforce authorization; do not treat an
> `audience` annotation as a security boundary until the host bridge's matching
> schema filtering and dispatch authorization are in place.

### The macro-generated `<fn>_host()` companion

`#[lingxia::native(...)]` is an attribute macro. In addition to wrapping the
function body, it generates a sibling
`fn <name>_host() -> lingxia::host::HostRegistrationEntry` that returns the
registration value the host addon hands to
`lingxia::host::register_host_entry`. You do not write this companion yourself
and you cannot rename it.

For `pick_document` above, the macro generates `pick_document_host()`. Use it
from `HostAddon::install_host_apis`:

```rust
impl lingxia::HostAddon for AppHostAddon {
    fn install_host_apis(&self) {
        lingxia::host::register_host_entry(pick_document_host());
        lingxia::host::register_host_entry(load_document_host());
        // …one register_host_entry call per #[lingxia::native] fn
    }
    fn start_services(&self) {}
}
```

If you forget to register the companion, the View call returns
`BRIDGE_METHOD_NOT_FOUND` — the route compiled but never made it into the
runtime's dispatch table. This is the most common cause of that error.

`stream` and `channel` variants of the macro (covered below) also generate
their respective `<fn>_host()` companion; register them the same way.

### Cancellation

Use `HostCancel` for async work that should stop when the page cancels the
request.

```rust
#[lingxia::native("editor.loadDocument")]
async fn load_document(
    input: PickDocumentInput,
    mut cancel: lingxia::host::HostCancel,
) -> lingxia::Result<String> {
    let work = async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Ok(format!("# {}", input.title))
    };

    lingxia::host::await_or_cancel(&mut cancel, work)
        .await
        .map_err(Into::into)
}
```

### Streams

Use `#[lingxia::native(..., stream)]` for incremental results.

```rust
#[derive(serde::Serialize)]
struct ExportProgress {
    progress: u32,
}

#[lingxia::native("editor.exportPdf", stream)]
async fn export_pdf(
    mut stream: lingxia::host::StreamContext<ExportProgress, String>,
) -> lingxia::Result<()> {
    for progress in [25, 60, 100] {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
        }

        if progress < 100 {
            stream.send(ExportProgress { progress })?;
        } else {
            stream.end("/exports/report.pdf".to_string())?;
        }
    }

    Ok(())
}
```

### Channels

Use `#[lingxia::native(..., channel)]` for bidirectional sessions.

```rust
#[derive(serde::Deserialize)]
struct EditorSessionInput {
    kind: String,
    payload: String,
}

#[derive(serde::Serialize)]
struct EditorSessionEvent {
    kind: String,
    payload: String,
}

#[lingxia::native("editor.session", channel)]
async fn editor_session(
    mut channel: lingxia::host::ChannelContext<EditorSessionInput, EditorSessionEvent>,
) -> lingxia::Result<()> {
    while let Some(message) = channel.recv().await? {
        match message {
            lingxia::host::ChannelMessage::Data(input) => {
                channel.send(EditorSessionEvent {
                    kind: input.kind,
                    payload: input.payload,
                })?;
            }
            lingxia::host::ChannelMessage::Close { .. } => break,
        }
    }

    Ok(())
}
```

## Generated Native Client

Generate the View client from the native Rust crate's `build.rs` with
`lingxia-native-codegen`. This keeps native route discovery next to the crate
that owns `#[lingxia::native]` handlers, and `cargo build` fails before the
lxapp is packaged if the generated client drifts.

The `lingxia new` templates already ship this wiring: a `build = "build.rs"`
manifest entry, a `lingxia-native-codegen` build-dependency (pinned to the
matching SDK version), and a `build.rs` that invokes the codegen entrypoints.
Start from a scaffolded native crate rather than reproducing the build script
here. For a hand-rolled crate, copy the template's `build.rs` and add the
build-dependency to your `Cargo.toml`; let the scaffolded template define the
version so it stays in lockstep with the SDK.

The generator scans `#[lingxia::native]` handlers and nearby struct DTOs. It
supports TypeScript module output (`.ts`) and browser-global output (`.js`).

The CLI sets `LINGXIA_NATIVE_CLIENT_OUT` to the framework-specific generated
client path during native cargo builds: React/Vue use `.lingxia/native.ts`;
HTML uses `.lingxia/native.js`.

Use it from View code:

```ts
import { native } from "@lingxia/native";

const path = await native.editor.pickDocument({ title: "meeting-notes" });

const stream = native.editor.exportPdf();
stream.onEvent((event) => console.log(event.progress));
const output = await stream.result;
console.log(output);

const channel = await native.editor.session();
channel.onMessage((event) => console.log(event));
channel.send({ kind: "cursor", payload: "{}" });
channel.close();
```

For plain HTML views, browser-global output is available at the fixed path:

```html
<script src="lingxia://lxapp/.lingxia/native.js"></script>
<script>
  window.native.editor.pickDocument({ title: "meeting-notes" }).then(console.log);
</script>
```

Generated clients handle bridge details internally. Module clients use the
high-level `@lingxia/bridge` helpers. Browser-global clients use
`LingXiaBridge.raw.*` because they already generate full `host.*` routes and
wrap stream/channel handles themselves.

## LingXia Facade Modules

Native route handlers reach shared SDK capabilities through the `lingxia::*`
facade modules — `lingxia::app`, `lingxia::file`, `lingxia::media`,
`lingxia::task`, `lingxia::update`, and friends. Each facade re-exports the
stable, supported surface for one capability area: app state and paths, file
and media pickers/downloads, runtime task spawning, and host-app update.

Import the facade, never the internal crates behind it (such as
`lingxia_logic` or `rong`). The facades are the contract; the internals drift.

```rust
#[lingxia::native("editor.cacheState")]
async fn cache_state(app: Arc<lingxia::LxApp>) -> lingxia::Result<String> {
    // Capability calls live behind the facade modules, e.g. lingxia::app::*.
    let state_file = lingxia::app::state_file_for(&app, "editor.json")?;
    Ok(state_file.to_string_lossy().into_owned())
}
```

Host display language is a product preference on that same facade. `Auto`
follows the system locale; `LanguageTag` accepts any canonical BCP-47 tag.
Every lxapp inherits the resolved tag from `display_language()`.

```rust
let preference = "zh-CN"
    .parse::<lingxia::app::DisplayLanguagePreference>()
    .expect("valid BCP-47 tag");
lingxia::app::set_display_language_preference(preference)?;
let state = lingxia::app::display_language_state();
let tag = lingxia::app::display_language();
```

For exact function names, parameters, and return types, read the crate docs
rather than relying on a list here — they track the code:

```sh
cargo doc -p lingxia --open
```

Provider authors should likewise import provider traits through
`lingxia::provider`, and media stream providers through `lingxia::media`.

## JS AppService Extensions

JS AppService extensions are optional and are only available with the
`standard` Cargo feature. They are scoped under `lingxia::js`.

```rust
#[cfg(feature = "standard")]
use lingxia::js::LxLogicExtension;

#[cfg(feature = "standard")]
struct WorkspaceDocsExtension;

#[cfg(feature = "standard")]
impl LxLogicExtension for WorkspaceDocsExtension {
    fn init(&self, ctx: &rong::JSContext) -> rong::JSResult<()> {
        let lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        let ns = rong::JSObject::new(ctx);
        ns.set("loadDocument", rong::JSFunc::new(ctx, load_document)?)?;
        lx.set("workspaceDocs", ns)?;
        Ok(())
    }
}

#[cfg(feature = "standard")]
fn load_document(_ctx: rong::JSContext, id: String) -> rong::JSResult<String> {
    Ok(format!("# {id}"))
}
```

Register the extension from `HostAddon::install_logic_extensions`:

```rust
#[cfg(feature = "standard")]
fn install_logic_extensions(&self) {
    lingxia::js::register_logic_extension(Box::new(WorkspaceDocsExtension));
}
```

When `features.appService: false` in `lingxia.yaml`, the generated host builds
without `standard`; `lingxia::js` is not public, and logic-enabled lxapps are
rejected at runtime. Lxapp manifests must use `logic`, not `appService`.

## Choosing The Surface

| Surface | Runs in | Called from | Use for |
| --- | --- | --- | --- |
| `#[lingxia::native]` | Rust host async runtime | View / generated native client | page-scoped native UI, file pickers, browser controls, native streams/channels |
| `lingxia::js` extension | JS AppService runtime | Logic layer as `lx.*` | business logic helpers, app-owned data APIs, synchronous JS-facing helpers |

Keep business state and app logic in AppService. Use native routes for
page-scoped host capabilities and native-owned workflows.
