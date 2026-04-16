# Native Development Guide

This guide covers how to extend LingXia from the native (Rust) side — exposing JS APIs to the Logic layer (`lx.*`) and exposing typed native capabilities to the View layer through the generated Native client.

For lxapp page development (JS side), see [LxApp Development Guide](./lxapp-guide.md).
For host app project setup, see [App Project](./app-project.md).

---

## Two Extension Surfaces

| Surface | Available in | JS access | Use case |
| --- | --- | --- | --- |
| Logic Extension | Logic layer (`index.ts`) | `lx.namespace.method()` | Business logic, data APIs, device APIs |
| Native Extension | View layer (`index.tsx`/`.vue`/HTML) | generated `native.namespace.method()` | Page-scoped native UI, file pickers, browser controls |

Both are registered from the same native library entry point before LxApp initialization.

---

## Extension Registration Entry Point

Every app has a native library crate (e.g. `examples/lingxia-lib`) that:

1. Re-exports platform FFI symbols from `lingxia`
2. Installs a host addon that can contribute bootstrap hooks, services, and Logic extensions

```rust
struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_logic_extensions(&self) {
        // Logic extensions (lx.* APIs)
        lingxia::register_logic_extension(Box::new(WorkspaceDocsExtension::new()));
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

## Native Extensions — Generated Native Client

Native extensions expose capabilities to the View layer (WebView). Define them with `#[lingxia::native(...)]`; the proc-macro still lives in a separate crate, but the developer-facing entry point is the `lingxia` facade.

The examples below use app-defined native namespaces such as `editor.*`. Those namespaces are chosen by your native addon; they are not built-in LingXia APIs. View code should call them through a generated Native client.

```toml
[dependencies]
lingxia = "..."
```

Use native extensions for page-scoped native operations:

- file pickers, native dialogs
- browser or webview controls
- snapshot/watch style native resources

### Unary Native Functions

The simplest host function returns a JSON-serializable value.

```rust
use std::sync::Arc;
use lingxia::LxApp;

#[derive(serde::Deserialize)]
struct PickDocumentInput {
    title: String,
}

#[lingxia::native("editor.pickDocument")]
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

#[lingxia::native("editor.pickDocument")]
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

### Stream Native Functions

For long-lived or incremental data, define a stream handler with `#[lingxia::native(..., stream)]`.

Stream handlers receive a typed `StreamContext<Event, Result>`. Emit events
with `send(...)`, finish with `end(...)`, and listen for cancellation with
`canceled()`.

```rust
#[derive(serde::Serialize)]
struct TickEvent {
    progress: u32,
}

#[lingxia::native("editor.exportPdf", stream)]
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

### Generate the View Client

For TS/React/Vue views, configure a module output:

```ts
export default {
  native: {
    rustDir: "native/src",
    out: "src/generated/native.ts",
  },
};
```

For plain HTML views, configure a browser global:

```ts
export default {
  staticDirs: ["public", "__lingxia"],
  native: {
    rustDir: "../src",
    out: "__lingxia/native.js",
  },
};
```

`lingxia build` and `lingxia dev` generate the client automatically when the lxapp build config contains `native`.

`native.rustDir` points to the Rust source directory that contains `#[lingxia::native]` functions. `native.out` is a root-relative output path. When `out` ends with `.ts`, LingXia writes an importable TS module; when it ends with `.js`, LingXia writes `window.native` and copies the generated static root into `dist/`.

### Usage in View

TS module:

```ts
import { native } from "./generated/native";

// Unary
const path = await native.editor.pickDocument({ title: "meeting-notes" });

// Stream
const stream = native.editor.exportPdf();
const off = stream.onEvent((event) => console.log(event.progress));
stream.onError((err) => console.error(err));
const result = await stream.result;
off();
console.log(result);
```

Plain HTML:

```html
<script src="lingxia://lxapp/__lingxia/native.js"></script>
<script>
  window.native.editor.pickDocument({ title: "meeting-notes" }).then(console.log);
</script>
```

### Channel Native Functions

For bidirectional sessions, define a channel handler with `#[lingxia::native(..., channel)]`.

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

#[lingxia::native("editor.session", channel)]
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
import { native } from "./generated/native";

const channel = await native.editor.session({ documentId: "welcome" });
channel.onMessage((event) => console.log(event));
channel.onClose((event) => console.log(event.code, event.reason));
channel.send({ kind: "cursor", payload: JSON.stringify({ line: 12, column: 4 }) });
channel.close();
```

### Page-Side Typing

Do not hand-write global native typings. The generated TS module owns route typing:

```ts
import { native } from "./generated/native";

const path = await native.editor.pickDocument({ title: "meeting-notes" });
```

---

## When To Use `lx.*` vs Native Host APIs

| | `lx.*` (Logic Extension) | Generated Native client |
| --- | --- | --- |
| Runs in | Logic JS runtime (native) | Async Rust, result sent to WebView |
| Accessible from | Logic layer only | View layer only |
| Best for | Business logic, editor state, document transforms | Page-scoped native UI, file I/O, browser controls |
| Invocation | Synchronous or async JS calls | Always async (Promise or Stream) |
| Registration | `register_logic_extension()` | `#[lingxia::native]` |

If a capability is business-facing, keep it in `lx`. If it is page-scoped and native-owned, expose it with `#[lingxia::native]` and call it through the generated Native client.

---

## Complete Example

An editor-style app usually splits ownership like this:

- `lx.workspaceDocs.*` is an app-defined Logic namespace for document loading, parsing, transforms, and draft state.
- `native.editor.*` is an app-defined native namespace for page-scoped file pickers, export flows, and other native-owned operations.
- `native.editor.session()` is an app-defined channel for long-lived sessions where the View and native side exchange incremental editor events over time.

Use that split to keep document state and business rules in Logic, while leaving native UI and file-system operations on the host side.
