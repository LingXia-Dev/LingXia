# lingxia

`lingxia` is the public Rust entry crate for LingXia native host apps.

It has two intentionally different API surfaces:

1. **Platform bootstrap / FFI APIs** used by generated Android, Apple, and Harmony host projects to start the LingXia runtime.
2. **Business APIs** used by app-owned Rust code to implement native features such as downloads, file dialogs, media, logging, and updates.

Most application crates should depend on `lingxia` only. Lower-level crates such as `lingxia-lxapp`, `lingxia-service`, `lingxia-transfer`, and platform crates are implementation layers unless you are working on the framework itself.

## 1. Platform Bootstrap / FFI

The platform modules are target-gated and are consumed by host app templates, SDK glue code, and generated platform projects.

| Module | Target | Purpose |
| --- | --- | --- |
| `lingxia::android` | Android | JNI entry points and Android runtime bridge |
| `lingxia::apple` | iOS / macOS | Swift bridge entry points and Apple runtime bridge |
| `lingxia::harmony` | HarmonyOS | NAPI entry points and Harmony runtime bridge |

App developers usually do not call these modules directly. The generated platform project wires them into the target app bootstrap.

Native libraries can register app-owned startup behavior through `HostAddon`:

```rust
struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_host_apis(&self) {
        // Register app-owned native routes if needed.
    }

    #[cfg(feature = "standard")]
    fn install_logic_extensions(&self) {
        // Register optional JS AppService extensions.
    }

    fn start_services(&self) {
        // Start app-owned background services.
    }
}

pub fn register_host_addon() {
    lingxia::register_host_addon(Box::new(AppHostAddon));
}
```

Generated host templates call this registration function from the platform entry point before runtime initialization.

## 2. Business APIs

Business APIs are the stable surface for app-owned Rust code. They are organized by developer task, not by internal crate layout.

| API | Use for |
| --- | --- |
| `#[lingxia::native]` | Expose Rust functions to lxapp pages |
| `lingxia::host` | Stream, channel, cancellation, and host route helpers |
| `lingxia::app` | App metadata and app-scoped state paths |
| `lingxia::file` | Download, upload, file picker, preview, reveal, and external open |
| `lingxia::media` | Camera, scanner, media picker, and media preview |
| `lingxia::log` | App logging stream and downstream logger registration |
| `lingxia::task` | Spawn async or blocking work on LingXia's runtime |
| `lingxia::update` | Host app update check/apply APIs for custom native UI |
| `lingxia::provider` | Provider traits for cloud, update, media, push, and other integrations |
| `lingxia::js` | Optional JS AppService extensions when `standard` is enabled |

### Native Page APIs

Use `#[lingxia::native]` to expose app-owned Rust functions to pages:

```rust
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct SaveInput {
    name: String,
    body: String,
}

#[derive(serde::Serialize)]
struct SaveOutput {
    path: String,
}

#[lingxia::native("notes.save")]
async fn save_note(
    app: Arc<lingxia::LxApp>,
    input: SaveInput,
) -> lingxia::Result<SaveOutput> {
    let path = lingxia::app::state_file_for(&app, &format!("{}.txt", input.name))?;
    let write_path = path.clone();
    lingxia::task::spawn_blocking(move || std::fs::write(write_path, input.body)).await??;

    Ok(SaveOutput {
        path: path.to_string_lossy().into_owned(),
    })
}
```

### File Download

`lingxia::file::download` downloads into the app-managed user cache. App code does not choose temp paths or internal cache directories.

```rust
let file = lingxia::file::download(
    &app,
    lingxia::file::DownloadRequest::new("https://example.com/video.mp4")
        .header("Authorization", "Bearer token"),
)
.await?;

println!("downloaded {} to {}", file.file_name(), file.path().display());
```

### File And Media UI

Use facade modules instead of platform-specific SDK calls:

```rust
let files = lingxia::file::choose_file(&app, request).await?;
let media = lingxia::media::choose_media(&app, request).await?;
lingxia::file::review(&app, open_request).await?;
```

### Host App Update

By default, LingXia can run the built-in host app update UX. Apps that need a custom native UI can opt into custom mode and drive the checked update handle:

```rust
lingxia::update::use_custom_host_app_update();

if let Some(update) = lingxia::update::check_host_app_update().await? {
    let info = update.info();
    println!(
        "update available: {} size={:?}",
        info.version(),
        info.package_size_bytes()
    );

    let mut apply = update.apply();
    while let Some(event) = apply.next().await {
        println!("update event: {event:?}");
    }
}
```

The update facade intentionally does not expose package paths, caller-supplied current versions, or separate download/install entry points.

## API Boundary

`lingxia` should stay small and app-oriented.

Use `lingxia` for:

- platform bootstrap exports required by generated host apps
- app-owned native route authoring
- stable business facades such as `app`, `file`, `media`, `log`, `task`, and `update`
- provider traits that app integrations implement

Do not use `lingxia` as a shortcut to runtime internals:

- page lifecycle orchestration belongs below the facade
- shell products such as downloads/settings management stay behind shell or logic APIs
- whole internal crates should not be re-exported just because they exist

For more detail, see [`docs/native-development.md`](../../docs/native-development.md)
and [`docs/internal/lingxia-facade-boundary.md`](../../docs/internal/lingxia-facade-boundary.md).
