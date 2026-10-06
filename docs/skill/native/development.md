# Native development

The Rust side of a host app: the `HostAddon`, `#[lingxia::native]` routes for
the View, the generated client, `lingxia::*` facades, and `lingxia::js`
extensions for Logic.

## Host addon

Every host library registers one `HostAddon` before the runtime starts:

```rust
struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    #[cfg(feature = "control")]
    fn install_product_cli(&self, cli: &mut lingxia::product_cli::ProductCli) {
        cli.command("workspace", "Manage workspaces", workspace_cli);
    }

    fn install_host_apis(&self) {
        // One register_host_entry call per #[lingxia::native] fn.
        lingxia::host::register_host_entry(pick_document_host());
    }

    fn issue_app_resource_grants(
        &self,
        authority: &mut lingxia::NativeHostRuntimeAuthority<'_>,
    ) {
        // Issue the session grant only after this native product's policy
        // or consent flow approved it. `requested()` is the permission
        // provider's allow (or the default allow).
        if user_approved_downloads_for(authority.app_id()) {
            authority.grant(lingxia::host::AppResourceGrant::Downloads);
        }
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

- `install_product_cli` is the only pre-runtime command hook; its request
  handler goes in `install_host_apis` ([Driving a shipped product](../app/agent-control.md)).
- `issue_app_resource_grants` runs once per session while it is created.
  Decide from `authority` alone: never prompt, block, or open/close an lxapp
  there. Privilege classes: [Permissions](./permissions.md).
- `install_navigation_routes` registers `{ kind: 'route' }` notification
  targets (native screens, not pages), sealed before the runtime starts.
- A launch cover per cold start: [Launch screen](./splash.md).

Platform entrypoints call the registration function; the scaffold contains
this wiring:

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

## Native routes

`#[lingxia::native("namespace.method")]` exposes a Rust function to the View:

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

Parameters, in order, all optional:

1. an authority: `Arc<lingxia::LxApp>`, or
   `lingxia::host::HostInvocationContext` to authorize app-owned or granted
   resources;
2. one JSON payload (`serde::Deserialize`);
3. `lingxia::host::HostCancel`, last.

Return `lingxia::Result<T>` with `T: serde::Serialize`. Streams and channels
take the same authority before their payload and final context.

`HostInvocationContext` comes from dispatch, never from JSON. Treat a payload
id only as a selector and authorize it against `app_scope()`:

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

### Registration

The macro generates `fn <name>_host() -> lingxia::host::HostRegistrationEntry`
(also for streams and channels). Never write or rename it; pass it to
`lingxia::host::register_host_entry` in `install_host_apis`. An unregistered
route returns `BRIDGE_METHOD_NOT_FOUND`. Duplicate route names are rejected.

### Route audience

`audience` restricts the caller class (see [Control app](../app/control-app.md));
omitted, it is `app-session-only`:

```rust
#[lingxia::native("host.setAccount", audience = "control-app-only")]
fn set_account() -> lingxia::Result<()> {
    Ok(())
}

#[lingxia::native("host.watch", stream, audience = "control-app-or-browser-only")]
async fn watch_host(
    mut stream: lingxia::host::StreamContext<lingxia::host::JsonValue>,
) -> lingxia::Result<()> {
    stream.end(())?;
    Ok(())
}
```

| String | Admits |
| --- | --- |
| `app-session-only` | any lxapp session (the default for `native`) |
| `any-authenticated` | any lxapp session, plus a browser control document |
| `control-app-only` | the ControlApp session only |
| `control-surface-only` | the host-bundled control surface session only |
| `browser-control-only` | a browser control document only |
| `control-app-or-browser-only` | the ControlApp session, plus a browser control document |

The classes are disjoint: `control-app-only` excludes the control surface.
`any-authenticated` limits the caller, not what the route may change. An
unknown or duplicate `audience` is a compile error.

### Cancellation

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

## Generated native client

The scaffolded native crate's `build.rs` runs `lingxia-native-codegen`, which
scans `#[lingxia::native]` handlers and their DTOs; `cargo build` fails if the
client drifts. A hand-rolled crate copies the template's `build.rs` and its
build-dependency. Lxapps configure no Rust paths.

The CLI passes the output path as `LINGXIA_NATIVE_CLIENT_OUT`: React/Vue get
`.lingxia/native.ts` (imported as `@lingxia/native`), HTML gets
`.lingxia/native.js` (copied into `dist/.lingxia/`). `lingxia build` generates
it when `lxapp.config.ts` declares `native`.

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

```html
<script src="lingxia://lxapp/.lingxia/native.js"></script>
<script>
  window.native.editor.pickDocument({ title: "meeting-notes" }).then(console.log);
</script>
```

## Facades

Reach SDK services through `lingxia::*` facades (`lingxia::app`,
`lingxia::file`, `lingxia::media`, `lingxia::task`, `lingxia::update`,
`lingxia::provider`), never internal crates such as `lingxia_logic` or `rong`.
Signatures: `cargo doc -p lingxia --open`.

```rust
#[lingxia::native("editor.cacheState")]
async fn cache_state(app: Arc<lingxia::LxApp>) -> lingxia::Result<String> {
    let state_file = lingxia::app::state_file_for(&app, "editor.json")?;
    Ok(state_file.to_string_lossy().into_owned())
}
```

The display language ([product settings](../lxapp/lx-api.md#product-settings)):

```rust
let tag = lingxia::app::display_language();
lingxia::app::watch_display_language(|tag| redraw_chrome_in(&tag));

let preference = "zh-CN"
    .parse::<lingxia::app::DisplayLanguagePreference>()
    .expect("valid BCP-47 tag");
lingxia::app::set_display_language_preference(preference)?;
```

Light/dark ([appearance](../lxapp/guide.md#appearance)). Native chrome and
every lxapp that has not pinned a scheme follow this setting:

```rust
if lingxia::app::appearance() == lingxia::app::ResolvedAppearance::Dark {
    use_dark_tray_icon();
}
lingxia::app::set_appearance_preference(lingxia::app::AppearancePreference::Dark)?;
```

`lingxia::app::banner::show` is the Rust form of the
[desktop banner](../lxapp/lx-api.md#desktop-banner). It blocks until the card
resolves: call it from a blocking worker, and never show a no-timeout prompt on
the macOS main thread.

## JS extensions

With the `standard` Cargo feature, `lingxia::js` adds `lx.<namespace>.*` to
Logic:

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

Register it in `install_logic_extensions` (see the addon above). Without
`features.appService` there is no `standard` feature and no `lingxia::js`
([`features`](../app/project.md#features)).

## Choosing the surface

| Surface | Runs in | Called from | Use for |
| --- | --- | --- | --- |
| `#[lingxia::native]` | Rust host async runtime | View / generated native client | page-scoped native UI, file pickers, browser controls, native streams/channels |
| `lingxia::js` extension | JS AppService runtime | Logic layer as `lx.*` | business logic helpers, app-owned data APIs, synchronous JS-facing helpers |

Keep business state in Logic; use native routes for host capabilities.

## Pitfalls

- Importing `lingxia_logic` or `rong` internals instead of `lingxia::*`
  facades (JS extensions excepted).
- Authority not first, or `HostCancel` not last, in a route signature.
- Writing the `<fn>_host()` companion yourself, or forgetting to register it
  (`BRIDGE_METHOD_NOT_FOUND`).
- Trusting an app id or path from the payload instead of `app_scope()`.
- Prompting or blocking inside `issue_app_resource_grants`.
