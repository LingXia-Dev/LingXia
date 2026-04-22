# Native Development Guide

This guide covers the Rust native surface for LingXia host apps.

Use this guide when you want to:

- expose Rust host APIs to pages with `#[lingxia::native]`
- add optional JS AppService extensions under `lingxia::js`
- call shared LingXia SDK services from Rust through facade modules such as
  `lingxia::app`, `lingxia::task`, `lingxia::downloads`, `lingxia::settings`,
  `lingxia::file`, `lingxia::media`, and `lingxia::update`

For lxapp page development, see [LxApp Development Guide](./lxapp-guide.md).
For host project configuration, see [App Project](./app-project.md).

## Host Addon

Every native host library registers a `HostAddon` before runtime initialization.
The addon is the place to install native routes, optional JS extensions, and
background services.

```rust
struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_host_apis(&self) {
        // Register #[lingxia::native] handlers here if your app collects
        // registrations manually.
    }

    #[cfg(feature = "js-lxapp")]
    fn install_logic_extensions(&self) {
        lingxia::js::register_logic_extension(Box::new(WorkspaceDocsExtension));
    }

    fn start_services(&self) {
        #[cfg(feature = "devtools")]
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

fn register_host_addon() {
    lingxia::register_host_addon(Box::new(AppHostAddon));
}
```

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
    let state_dir = lingxia::app::state_dir_for(&app);
    Ok(state_dir
        .join(format!("{}.md", input.title))
        .to_string_lossy()
        .into_owned())
}
```

Supported parameters:

- optional first parameter: `Arc<lingxia::LxApp>`
- optional JSON payload parameter
- optional last parameter: `lingxia::host::HostCancel`

Rules:

- `Arc<lingxia::LxApp>` must be first when present.
- `HostCancel` must be last when present.
- Only one JSON payload parameter is supported.
- Payload types must implement `serde::Deserialize`.
- Return values must implement `serde::Serialize`.
- Handler errors should use `lingxia::Result`.

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
        lingxia::task::tokio::time::sleep(std::time::Duration::from_millis(300)).await;
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
        lingxia::task::tokio::select! {
            _ = stream.canceled() => return Ok(()),
            _ = lingxia::task::tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
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

The CLI can generate a typed client for View code from Rust native routes.

For React/Vue/TypeScript views, prefer module output:

```ts
export default {
  native: {
    rustDir: "native/src",
    out: "src/generated/native.ts",
  },
};
```

Use it from View code:

```ts
import { native } from "./generated/native";

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

For plain HTML views, browser-global output is available:

```ts
export default {
  staticDirs: ["public", "__lingxia"],
  native: {
    rustDir: "../src",
    out: "__lingxia/native.js",
  },
};
```

```html
<script src="lingxia://lxapp/__lingxia/native.js"></script>
<script>
  window.native.editor.pickDocument({ title: "meeting-notes" }).then(console.log);
</script>
```

Generated clients handle bridge details internally. Module clients use the
high-level `@lingxia/bridge` helpers. Browser-global clients use
`LingXiaBridge.raw.*` because they already generate full `host.*` routes and
wrap stream/channel handles themselves.

## LingXia Facade Modules

Native route handlers should use facade modules instead of internal crates:

```rust
let config = lingxia::app::config();
let state_dir = lingxia::app::state_dir_for(&app);
let downloads = lingxia::downloads::snapshot(&app)?;
let download_dir = lingxia::settings::effective_download_dir(&app);
let media = lingxia::media::choose_media(&app, request).await?;
let files = lingxia::file::choose_file(&app, request).await?;
```

Use `lingxia::task` for runtime helpers:

```rust
let value = lingxia::task::spawn_blocking(|| expensive_work()).await?;
lingxia::task::spawn(async move {
    // background work
});
```

Provider authors should import provider traits through `lingxia::provider`.
Media stream providers should import stream traits through `lingxia::media`.

## JS AppService Extensions

JS AppService extensions are optional and are only available with the
`js-lxapp` Cargo feature. They are scoped under `lingxia::js`.

```rust
#[cfg(feature = "js-lxapp")]
use lingxia::js::LxLogicExtension;

#[cfg(feature = "js-lxapp")]
struct WorkspaceDocsExtension;

#[cfg(feature = "js-lxapp")]
impl LxLogicExtension for WorkspaceDocsExtension {
    fn init(&self, ctx: &rong::JSContext) -> rong::JSResult<()> {
        let lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        let ns = rong::JSObject::new(ctx);
        ns.set("loadDocument", rong::JSFunc::new(ctx, load_document)?)?;
        lx.set("workspaceDocs", ns)?;
        Ok(())
    }
}

#[cfg(feature = "js-lxapp")]
fn load_document(_ctx: rong::JSContext, id: String) -> rong::JSResult<String> {
    Ok(format!("# {id}"))
}
```

Register the extension from `HostAddon::install_logic_extensions`:

```rust
#[cfg(feature = "js-lxapp")]
fn install_logic_extensions(&self) {
    lingxia::js::register_logic_extension(Box::new(WorkspaceDocsExtension));
}
```

When `features.appService: false` in `lingxia.yaml`, the generated host builds
without `js-lxapp`; `lingxia::js` is not public, and logic-enabled lxapps are
rejected at runtime. Lxapp manifests must use `logic`, not `appService`.

## Choosing The Surface

| Surface | Runs in | Called from | Use for |
| --- | --- | --- | --- |
| `#[lingxia::native]` | Rust host async runtime | View / generated native client | page-scoped native UI, file pickers, browser controls, native streams/channels |
| `lingxia::js` extension | JS AppService runtime | Logic layer as `lx.*` | business logic helpers, app-owned data APIs, synchronous JS-facing helpers |

Keep business state and app logic in AppService. Use native routes for
page-scoped host capabilities and native-owned workflows.
