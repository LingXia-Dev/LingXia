# Native development

The Rust side of a host app: the `HostAddon`, `#[lingxia::native]` routes
(callable from Logic and WebView Views), synchronous routes, route state
(`PageState<T>`, `LxAppState<T>`, `Host<A>`), the generated `@lingxia/native`
client, and `lingxia::*` facades.

## Host addon

A host library's `lib.rs` names its `HostAddon` once; `lingxia::host!` registers
it and every route in the crate, and exports the entrypoint each platform SDK
calls (Harmony's needs `napi-derive-ohos`, as scaffolded):

```rust
lingxia::host!(AppHostAddon);

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    #[cfg(feature = "control")]
    fn install_product_cli(&self, cli: &mut lingxia::product_cli::ProductCli) {
        cli.command("workspace", "Manage workspaces", workspace_cli);
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
}
```

`host!` starts the `lingxia dev` session bridge when the crate's `devtools`
feature is on; `start_services` is only for the product's own services. Its
leading name is the addon's type; pass a value after it when the addon has
fields: `lingxia::host!(AppHostAddon::new())`.

- `install_product_cli` is the only pre-runtime command hook; its request
  handler goes in `install_host_apis` ([Driving a shipped product](../app/agent-control.md)).
- `issue_app_resource_grants` runs once per session while it is created.
  Decide from `authority` alone: never prompt, block, or open/close an lxapp
  there. Privilege classes: [Permissions](./permissions.md).
- `install_navigation_routes` registers `{ kind: 'route' }` targets (native
  screens) that a notification, menu item, or tray may name; sealed before the
  runtime starts.
- `select_campaign` picks a campaign screen per cold start: [Launch screen](./splash.md).

## Native routes

`#[lingxia::native("namespace.method")]` exposes a Rust function to Logic and to
a WebView (React/HTML) View. A GPU View cannot call it (`E_PERMISSION_DENIED`);
call it from a Logic action.

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

Parameters are recognized by type, in any order, all optional:

- an authority: `Arc<lingxia::LxApp>`, or
  `lingxia::host::HostInvocationContext` to authorize app-owned or granted
  resources;
- `lingxia::host::HostCancel` (unary only);
- route state: `PageState<T>` (unary only), `lingxia::LxAppState<T>`,
  `lingxia::Host<A>` ([Route state](#route-state));
- `lingxia::Progress<E>` (required by `stream`) or `ChannelContext` (required by
  `channel`);
- anything else is the JSON payload (`serde::Deserialize`). Only one: a
  second is a compile error, so put multiple values in one struct.

Return `T: serde::Serialize`, or `Result<T, E>` for a route that can fail: `E` is
`lingxia::Error`, a `#[derive(lingxia::NativeError)]` enum, or `anyhow::Error`
(fails as `E_INTERNAL`).

A plain `fn` runs on a blocking thread, so blocking I/O is fine there; its
arguments and result must be `Send`, and a panic fails the call as `E_INTERNAL`.
An `async fn` runs on the async runtime and must not block.

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

### Errors

A failure reaches Logic and a WebView as `{ code, message, reason?, data? }`.
`lingxia::Error` carries the framework's `E_*` codes; for the app's own, derive
`lingxia::NativeError` on an enum tagged `code`:

```rust
#[derive(Debug, thiserror::Error, serde::Serialize, lingxia::NativeError)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
enum NotesError {
    #[error("note {id} not found")]
    NotFound { id: i64 },
    #[error(transparent)]
    #[serde(skip)]
    #[native_error(internal)]
    Io(#[from] std::io::Error),
}

#[lingxia::native("notes.delete")]
async fn delete_note(id: i64) -> Result<(), NotesError> {
    Err(NotesError::NotFound { id })
}
```

Fields arrive as `data`; a `#[native_error(internal)]` variant is sent as
`E_INTERNAL`; a code starting with `E_` is a compile error. The client exports `NotesError` as a
union, and `isNativeError(e, "NOT_FOUND")` narrows `e.data` to `{ id: number }`.
`message` is English for developers: show users text chosen by `code`.

### Registration

The scaffold's `build.rs` (`lingxia_native_codegen::build()`) registers every
`#[lingxia::native]` fn in `src/`, in any module, with its `#[cfg]`s; routes are
installed before `install_host_apis`. A route in a module the crate root cannot
reach (nested inside a private module) fails the build with the fix, as do a
duplicate route name, a route with no namespace, and a second `route_defaults!`.

### Route names and defaults

`lingxia::route_defaults!` at the top of a file sets the namespace and audience
for the routes after it in that file (not in inline or child modules; one per
file). A route without a name is the prefix plus its fn name in camelCase
(`set_pinned` → `bookmarks.setPinned`); a name without a dot goes under the
prefix; a name with a dot is absolute. Two routes resolving to one name fail the
build with both locations:

```rust
lingxia::route_defaults!(prefix = "bookmarks", audience = "control-surface-only");

#[lingxia::native]                 // "bookmarks.setPinned"
fn set_pinned(input: PinInput) -> lingxia::Result<()> { Ok(()) }

#[lingxia::native("export_html")]  // "bookmarks.export_html"
fn export() -> lingxia::Result<String> { Ok(String::new()) }
```

### Route audience

`audience` restricts the caller class (see [Control app](../app/control-app.md));
omitted on the route and in `route_defaults!`, it is `app-session-only`:

```rust
#[lingxia::native("host.setAccount", audience = "control-app-only")]
fn set_account() -> lingxia::Result<()> {
    Ok(())
}

#[lingxia::native("host.status", audience = "control-app-or-surface")]
fn host_status() -> lingxia::Result<String> {
    Ok("ready".into())
}
```

| String | Admits |
| --- | --- |
| `app-session-only` | any lxapp session (the default for `native`) |
| `control-app-only` | the ControlApp session only |
| `control-surface-only` | any host-bundled control surface session |
| `control-app-or-surface` | the ControlApp session and every control surface session |

`control-app-only` excludes control surfaces. An audience limits the caller,
not what the route may change. An unknown or duplicate `audience` is a compile
error; a route's own `audience` overrides the file default.

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
async fn export_pdf(progress: lingxia::Progress<ExportProgress>) -> lingxia::Result<String> {
    for done in [25, 60] {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        progress.send(ExportProgress { progress: done })?;
    }
    Ok("/exports/report.pdf".to_string())
}
```

The client gets a `NativeTask<ExportProgress, string>`: `for await` the events,
`result` is the return value. On cancel an `async fn` is dropped at its next
`.await` and the call fails with `E_ABORT`; in a plain `fn`, `send` fails with
`E_ABORT`, so check `progress.is_canceled()` between steps that do not send.

### Watches

A `watch` route returns a stream whose first item is the current state:

```rust
#[lingxia::native("notes.watch", watch)]
fn watch_notes(
    app: Arc<lingxia::LxApp>,
) -> lingxia::Result<impl futures::Stream<Item = NotesEvent> + Send + 'static> {
    let changes = subscribe_notes(); // before the read, so no change is lost
    let state = NotesEvent::Snapshot { notes: load_notes(&app)? };
    Ok(futures::stream::once(async move { state }).chain(changes))
}
```

`page.native.notes.watch(onEvent, { signal: page.signal })` returns a stop
function. When the stream ends or fails, the client calls the route again
(after 1 s, doubling to 15 s), so end the stream to resync. `E_NOT_FOUND`,
`E_PERMISSION_DENIED`, `E_INVALID_ARG`, `E_NOT_SUPPORTED` and
`E_CAPABILITY_MISSING` stop it.

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

`lingxia build`, `check`, `dev` and `gen types` scan the routes and their DTOs
and write the lxapp's clients, imported as `@lingxia/native`:
`.lingxia/native.logic.ts` for Logic, and `.lingxia/native.view.ts` (React) or
`.lingxia/native.js` (HTML, copied into `dist/.lingxia/`) for a WebView View.
An lxapp listed in `lingxia.yaml` calls the `rustLibDir` crate; an lxapp inside
a Cargo package calls that package. Lxapps configure no Rust paths.
A GPU View (`.tsx`) cannot import `@lingxia/native`; call routes from a Logic
action or `setup(page)`.

```ts
import { definePage } from "@lingxia/logic";
import { native } from "@lingxia/native";

export default definePage({
  data: { path: "" },
  actions: (page) => ({
    async pick() {
      const path = await native.editor.pickDocument({ title: "meeting-notes" });
      page.update((d) => { d.path = path; });
    },
    async *exportPdf() {
      return yield* native.editor.exportPdf();
    },
  }),
});
```

### Synchronous routes

`#[lingxia::native("ns.method", sync)]` runs a plain `fn` on the Logic JS thread
and returns `T`, not `Promise<T>`, in Logic. Keep it short and I/O-free
(formatting, validation, parsing); dev builds warn above 8 ms. It cannot take
`HostCancel`.

### Route state

- Per page: `PageState<T>`, below.
- Per lxapp: `lingxia::LxAppState<T>` gives `&T`, opened by `T: lingxia::FromLxApp`
  on first use and dropped when the lxapp closes. Concurrent first calls share
  one `open`; a failed `open` fails that call and the next call retries. There
  is no lock, so `T` brings its own interior mutability. Not on `sync` routes.
  State that starts empty needs no `open`: `#[derive(Default, lingxia::FromLxApp)]`.
- Per host: `lingxia::Host<A>` gives `&A`, the addon passed to `host!`; naming
  another type is a compile error.

```rust
pub struct Notes { db: Pool }

impl lingxia::FromLxApp for Notes {
    async fn open(app: &lingxia::LxApp) -> lingxia::Result<Self> {
        Ok(Notes { db: Pool::open(app.app_data_dir().join("notes.db"))? })
    }
}

#[lingxia::native("notes.list")]
async fn list(notes: lingxia::LxAppState<Notes>) -> lingxia::Result<Vec<Note>> {
    notes.db.list().await
}
```

### Per-page Rust state (`PageState<T>`)

A unary route (`async fn`, plain `fn` or `sync`) may take
`mut state: lingxia::PageState<MyState>` (`MyState: Default + Send + 'static`):
one value per page instance, and calls on the same page for the same `T`
serialize; a `sync` route fails instead of waiting while an async call holds it.
Call such routes through `page.native` in `setup(page)` or `actions`; module-level
`native` fails. The build bundles the Logic client, so `page.native` needs no
import. Removing the page drops the state, cancels its in-flight calls
(`HostCancel`) and closes its channels; so does a throwing `setup`.

### Supported DTO types & serde attributes

Unresolved or unsupported DTO types fail codegen with the type name and source
location rather than emitting `unknown`. That includes `&'static str` (use
`String`), `serde_json::Map` (use `HashMap<String, Value>`) and generic structs.

| Rust / serde | TypeScript |
| --- | --- |
| `String`, `&str`, `char`, `bool`, numeric primitives, `()` | `string`, `boolean`, `number`, `void` |
| `Option<T>`, `Vec<T>`, `HashSet<T>`, `BTreeSet<T>`, `[T; N]`, `(A, B)` | `T \| null`, `T[]`, `[A, B]` |
| `HashMap<K, V>`, `BTreeMap<K, V>`, `serde_json::Value` | `Record<string, V>`, `unknown` |
| `Uuid`, `DateTime`, `NaiveDate`, `Url`, `PathBuf`, `IpAddr`, `SocketAddr` | `string` |
| `struct` (named, tuple, unit), `type` alias, path-qualified DTOs | `interface` / `type` |
| `enum` (unit, external, `tag`, `tag + content`, `untagged`) | string literal or discriminated union |
| `rename`, `rename_all`, `rename_all_fields`, `default`, `skip`, `skip_serializing_if`, `flatten` | applied to wire names, optionality, intersections |

TS names are exactly serde's wire names: put `#[serde(rename_all = "camelCase")]`
on multi-word DTOs (codegen warns when it is missing). `Option`/`default`
fields are optional on input; output fields only with `skip_serializing_if`.
`u64`/`i64` fields warn: JS numbers are exact only to 2^53. `#[cfg(test)]` items
are skipped; a `#[cfg]`-gated route is typed everywhere, with a doc note, and
fails with `E_NOT_FOUND` where it is compiled out.

```html
<!-- pages/<name>/index.html -->
<script src="../../.lingxia/native.js"></script>
<script>
  window.native.editor.pickDocument({ title: "meeting-notes" }).then(console.log);
</script>
```

## Facades

Reach SDK services through `lingxia::*` facades (such as `lingxia::app`,
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

Values the host follows at run time — foreground, display language, light/dark
and both preferences — share one shape: a getter, `watch_*` and `*_changes()`.

| Value | Read | Follow |
|---|---|---|
| foreground (process, not an lxapp) | `is_foreground()` | `watch_foreground` / `foreground_changes` |
| [display language](../lxapp/core.md#product-settings) | `display_language()` | `watch_display_language` / `display_language_changes` |
| its preference | `display_language_preference()` | `watch_display_language_preference` / `…_changes` |
| [light/dark](../lxapp/guide.md#appearance) | `appearance()` | `watch_appearance` / `appearance_changes` |
| its preference | `appearance_preference()` | `watch_appearance_preference` / `…_changes` |

```rust
// Called with the current value, then once per change. Keep the Subscription
// (dropping it unsubscribes) or `.detach()` it for the life of the process.
let subscription = lingxia::app::watch_foreground({
    let engine = engine.clone();
    move |foreground| engine.set_power_saving(!foreground)
});

// Async: the current value, then the latest after each change. Never ends.
let mut changes = lingxia::app::appearance_changes();
loop {
    let appearance = changes.next().await;
    tray.set_dark(appearance == lingxia::app::ResolvedAppearance::Dark);
}

let zh = "zh-CN".parse().expect("valid BCP-47 tag");
lingxia::app::set_display_language_preference(zh)?;
lingxia::app::set_appearance_preference(lingxia::app::AppearancePreference::Dark)?;
```

Callbacks (`FnMut`) run one at a time, in order, on a LingXia thread per area
(foreground, language, appearance) — never on the main thread and with no
LingXia lock held, so they may call any `lingxia` API. Keep them short; a slow
one delays the rest of its area. Dropping a `Subscription` never waits for a
running call. On iOS and HarmonyOS the `false`
foreground callbacks run inside an OS background grace period that LingXia holds
until they return, so flush there; `*_changes()` streams get no grace. A
transient inactive state (system alert, permission prompt) is still foreground,
desktop hosts are always foreground, an Android process started without an
activity (push, `JobService`) reads foreground until one starts, and switching
lxapps fires their `onShow`/`onHide`, not this.

`lingxia::app::banner::show` is the Rust form of the desktop banner
(`lx.host.banner`). It blocks until the card resolves: call it from a plain
`fn` route or a blocking worker, and never show a no-timeout prompt on the
macOS main thread.

## Choosing the surface

| Surface | Runs in | Called from | Use for |
| --- | --- | --- | --- |
| `#[lingxia::native]` | async runtime (`async fn`) or a blocking thread (plain `fn`) | Logic (`@lingxia/native`, `page.native`) & WebView View | host capabilities, I/O, streams, channels, heavy computation, `PageState<T>` |
| `#[lingxia::native(..., sync)]` | Logic JS thread | Logic (`@lingxia/native`, `page.native`) | short, non-blocking, I/O-free pure helpers (< 8 ms) |

## Pitfalls

- Importing `lingxia_logic` or `rong` internals instead of `lingxia::*`
  facades.
- Registering routes or exporting platform entrypoints by hand next to
  `lingxia::host!`.
- Trusting an app id or path from the payload instead of `app_scope()`.
- Prompting or blocking inside `issue_app_resource_grants`.
