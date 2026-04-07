# Native Development Guide

This guide covers how to extend LingXia from the native (Rust) side — exposing JS APIs to the Logic layer (`lx.*`) and exposing host capabilities to the View layer (`window.host.*`).

For lxapp page development (JS side), see [LxApp Development Guide](./lxapp-guide.md).
For host app project setup, see [App Project](./app-project.md).

---

## Two Extension Surfaces

| Surface | Available in | JS access | Use case |
| --- | --- | --- | --- |
| Logic Extension | Logic layer (`index.ts`) | `lx.namespace.method()` | Business logic, data APIs, device APIs |
| Host Extension | View layer (`index.tsx`/`.vue`) | `window.host.namespace.method()` | Page-scoped native UI, file pickers, browser controls |

Both are registered from the same native library entry point before LxApp initialization.

---

## Extension Registration Entry Point

Every app has a native library crate (e.g. `examples/lingxia-lib`) that:

1. Re-exports platform FFI symbols from `lingxia`
2. Installs a host addon that can contribute bootstrap hooks, services, and JS APIs

```rust
struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_logic_extensions(&self) {
        // Logic extensions (lx.* APIs)
        lingxia::register_logic_extension(Box::new(WorkspaceDocsExtension::new()));
    }

    fn install_host_apis(&self) {
        // Host extensions (window.host.* APIs)
        lingxia::register_hosts![pick_document, export_pdf, editor_session];
    }
}
```

This addon is installed from platform-specific host entrypoints:

```rust
// Android (JNI)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_app_MainActivity_nativeInstallHostAddon(...) {
    lingxia::install_host_addon(Box::new(AppHostAddon));
}

// iOS/macOS (C export)
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_install_host_addon() {
    lingxia::install_host_addon(Box::new(AppHostAddon));
}

// Harmony (NAPI)
#[napi_derive_ohos::napi]
pub fn lingxia_install_host_addon() {
    lingxia::install_host_addon(Box::new(AppHostAddon));
}
```

---

## Logic Extensions — `lx.*`

Logic extensions add APIs to the global `lx` object available in the Logic layer JS runtime (JavaScriptCore / QuickJS).

The example below uses an app-defined namespace, `lx.workspaceDocs.*`. That namespace is registered by your extension code; it is not a built-in LingXia API.

### Implementing `LxLogicExtension`

```rust
use lingxia::LxLogicExtension;
use rong::{JSContext, JSFunc, JSObject, JSResult};

pub struct WorkspaceDocsExtension;

impl WorkspaceDocsExtension {
    pub fn new() -> Self { Self }
}

impl LxLogicExtension for WorkspaceDocsExtension {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        let lx = ctx.global().get::<_, JSObject>("lx")?;
        let ns = JSObject::new(ctx);

        ns.set("loadDocument", JSFunc::new(ctx, load_document)?)?;
        ns.set("saveDraft", JSFunc::new(ctx, save_draft)?)?;

        lx.set("workspaceDocs", ns)?;
        Ok(())
    }
}

fn load_document(_ctx: JSContext, doc_id: String) -> JSResult<String> {
    Ok(format!("# Document {doc_id}\n\nLoaded in native logic runtime."))
}

fn save_draft(_ctx: JSContext, markdown: String) -> JSResult<bool> {
    let _ = markdown;
    Ok(true)
}
```

### Registration

```rust
fn do_register_extensions() {
    lingxia::register_logic_extension(Box::new(WorkspaceDocsExtension::new()));
}
```

### Usage in Logic

```ts
// pages/home/index.ts
Page({
  data: { markdown: "" },

  onLoad: function () {
    const markdown = lx.workspaceDocs.loadDocument("welcome");
    this.setData({ markdown });
  },
});
```

### Key points

- `init()` is called once per LxApp JavaScript context creation.
- The `lx` global object is already created by the runtime before your extension runs.
- Use nested namespaces (`lx.workspaceDocs.*`) to avoid collisions with built-in APIs.
- Function parameters and return types use `rong` JS bindings — types implementing `FromJSObj` / `ToJSObj` are supported.
- Access the `LxApp` instance from within functions via `LxApp::from_ctx(&ctx)`.

### Flat API registration

For simpler cases, you can register functions directly on `lx` without a sub-namespace:

```rust
use lingxia::lx;

impl LxLogicExtension for WorkspaceDocsExtension {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        lx::register_js_api(ctx, "myCustomApi", JSFunc::new(ctx, my_func)?)?;
        Ok(())
    }
}
```

This makes the function available as `lx.myCustomApi()`.

---

## Host Extensions — `window.host.*`

Host extensions expose capabilities to the View layer (WebView). Define them with `#[lingxia::host(...)]`; the proc-macro still lives in a separate crate, but the developer-facing entry point is the `lingxia` facade.

The examples below use app-defined host namespaces such as `window.host.editor.*`. Those namespaces are chosen by your host addon; they are not built-in LingXia APIs.

```toml
[dependencies]
lingxia = "..."
```

Use host extensions for page-scoped native operations:

- file pickers, native dialogs
- browser or webview controls
- snapshot/watch style native resources

### Unary Host Functions

The simplest host function returns a JSON-serializable value.

```rust
use std::sync::Arc;
use lingxia::LxApp;

#[derive(serde::Deserialize)]
struct PickDocumentInput {
    title: String,
}

#[lingxia::host("editor.pickDocument")]
async fn pick_document(
    _lxapp: Arc<LxApp>,
    input: PickDocumentInput,
) -> lingxia::host::HostResult<String> {
    Ok(format!("/documents/{}.md", input.title))
}
```

Supported function parameters:

- optional first parameter: `Arc<LxApp>`
- optional JSON payload parameter
- optional last parameter: `lingxia::host::HostCancel`

Supported forms:

- `fn foo() -> HostResult<T>`
- `async fn foo() -> HostResult<T>`
- `fn foo(input: Input) -> HostResult<T>`
- `async fn foo(app: Arc<LxApp>, input: Input) -> HostResult<T>`
- `async fn foo(app: Arc<LxApp>, input: Input, cancel: HostCancel) -> HostResult<T>`

Rules:

- `Arc<LxApp>` must be the first parameter when present
- `HostCancel` must be the last parameter when present
- only one JSON payload parameter is supported
- return values must be `serde::Serialize`
- payload types must be `serde::Deserialize`

### Cancellation

For async work that should stop when the page cancels the request, accept `HostCancel`:

```rust
use std::time::Duration;

#[lingxia::host("editor.pickDocument")]
async fn pick_document(
    input: PickDocumentInput,
    mut cancel: lingxia::host::HostCancel,
) -> lingxia::host::HostResult<String> {
    lingxia::host::await_or_cancel(&mut cancel, async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(format!("/documents/{}.md", input.title))
    })
    .await
}
```

### Stream Host Functions

For long-lived or incremental data, define a stream handler with `#[lingxia::host(..., stream)]`.

Stream handlers receive a typed `StreamContext<Event, Result>`. Emit events
with `send(...)`, finish with `end(...)`, and listen for cancellation with
`canceled()`.

```rust
#[derive(serde::Serialize)]
struct TickEvent {
    progress: u32,
}

#[lingxia::host("editor.exportPdf", stream)]
async fn export_pdf(
    mut stream: lingxia::host::StreamContext<TickEvent, String>,
) -> lingxia::host::HostResult<()> {
    for progress in [25u32, 60, 100] {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
        }

        if progress < 100 {
            stream.send(TickEvent { progress })?;
            continue;
        }

        return stream.end("/exports/report.pdf".to_string());
    }

    Ok(())
}
```

### Registration

Host functions must be explicitly registered:

```rust
fn do_register_extensions() {
    lingxia::register_hosts![pick_document, export_pdf, editor_session];
}
```

### Usage in View

```ts
// Unary
const path = await window.host.editor.pickDocument({ title: "meeting-notes" });

// Stream
const stream = window.host.editor.exportPdf();
stream.on("data", (event) => console.log(event.progress));
stream.on("end", (result) => console.log(result));
stream.on("error", (err) => console.error(err));

// Stream also supports async iteration.
for await (const event of window.host.editor.exportPdf()) {
  console.log(event.progress);
}
```

### Channel Host Functions

For bidirectional sessions, define a channel handler with `#[lingxia::host(..., channel)]`.

```rust
#[derive(serde::Deserialize)]
struct EditorSessionOpenParams {
    documentId: String,
}

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

#[lingxia::host("editor.session", channel)]
async fn editor_session(
    params: EditorSessionOpenParams,
    mut ch: lingxia::host::ChannelContext<EditorSessionInput, EditorSessionEvent>,
) -> lingxia::host::HostResult<()> {
    let _document_id = params.documentId;

    while let Some(message) = ch.recv().await? {
        match message {
            lingxia::host::ChannelMessage::Data(input) => {
                ch.send(EditorSessionEvent {
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

Usage in View:

```ts
const channel = await window.host.channel.editor.session({ documentId: "welcome" });
channel.on("data", (event) => console.log(event));
channel.on("close", (code, reason) => console.log(code, reason));
channel.send({ kind: "cursor", payload: JSON.stringify({ line: 12, column: 4 }) });
channel.close();
```

### Page-Side Typing

Create a `.d.ts` file to type `window.host` namespaces:

```ts
import type { HostChannelApi, HostApi, LxChannel, LxStream } from "@lingxia/bridge";

interface PickDocumentInput { title: string }
interface ExportProgressEvent { progress: number }
interface EditorSessionEvent { kind: string; payload: string }

interface EditorHostApi {
  pickDocument(params: PickDocumentInput): Promise<string>;
  exportPdf(): LxStream<ExportProgressEvent, string>;
}

interface EditorHostChannelApi {
  session(params: { documentId: string }): Promise<LxChannel<EditorSessionEvent, EditorSessionInput>>;
}

declare module "@lingxia/bridge" {
  interface HostApi {
    editor: EditorHostApi;
  }

  interface HostChannelApi {
    editor: EditorHostChannelApi;
  }
}
```

---

## When To Use `lx.*` vs `window.host.*`

| | `lx.*` (Logic Extension) | `window.host.*` (Host Extension) |
| --- | --- | --- |
| Runs in | Logic JS runtime (native) | Async Rust, result sent to WebView |
| Accessible from | Logic layer only | View layer only |
| Best for | Business logic, editor state, document transforms | Page-scoped native UI, file I/O, browser controls |
| Invocation | Synchronous or async JS calls | Always async (Promise or Stream) |
| Registration | `register_logic_extension()` | `register_hosts![]` |

If a capability is business-facing, keep it in `lx`. If it's page-scoped and host-owned, use `host`.

---

## Complete Example

An editor-style app usually splits ownership like this:

- `lx.workspaceDocs.*` is an app-defined Logic namespace for document loading, parsing, transforms, and draft state.
- `window.host.editor.*` is an app-defined host namespace for page-scoped file pickers, export flows, and other host-owned operations.
- `window.host.channel.editor.*` is an app-defined channel namespace for long-lived sessions where the View and native side exchange incremental editor events over time.

Use that split to keep document state and business rules in Logic, while leaving native UI and file-system operations on the host side.
