//! Rust Logic Demo — Shape C native library (no JS runtime).
//!
//! Every bit of "Logic" lives here in Rust and is exposed to the HTML view
//! through `#[lingxia::native]` routes. The View calls them via the generated
//! `window.native.demo.*` browser client (`home/.lingxia/native.js`). There is
//! no JS AppService: `features.appService: false` in `lingxia.yaml` keeps the JS
//! runtime out of the binary entirely.
//!
//! Routes (all under the `demo` namespace):
//!   - `demo.appInfo`     unary, sync   — host metadata from `lingxia::app::*`
//!   - `demo.networkInfo` unary, async  — wraps `lingxia::network::current`
//!   - `demo.download`    unary, async  — wraps `lingxia::file::download` (+cancel)
//!   - `demo.chooseMedia` unary, async  — wraps `lingxia::media::choose_media`
//!   - `demo.ticker`      stream        — Rust → page event push + final summary
//!   - `demo.echo`        channel       — bidirectional echo

use std::sync::Arc;
use std::time::Duration;

use lingxia::host::{ChannelContext, ChannelMessage, HostResult, StreamContext};
use lingxia::{LxApp, Result};

// Re-export platform FFI symbols from lingxia (Apple: iOS/macOS).
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use lingxia::apple::*;

// ---------------------------------------------------------------------------
// demo.appInfo — unary, synchronous. Host metadata, no payload, no LxApp arg.
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    product_name: Option<String>,
    version: Option<String>,
    home_app_id: Option<String>,
}

#[lingxia::native("demo.appInfo")]
fn demo_app_info() -> Result<AppInfo> {
    Ok(AppInfo {
        product_name: lingxia::app::product_name().map(str::to_owned),
        version: lingxia::app::product_version().map(str::to_owned),
        home_app_id: lingxia::app::home_app_id().map(str::to_owned),
    })
}

// ---------------------------------------------------------------------------
// demo.networkInfo — async, takes Arc<LxApp>. Wraps lingxia::network::current.
// ---------------------------------------------------------------------------

#[lingxia::native("demo.networkInfo")]
async fn demo_network_info(app: Arc<LxApp>) -> Result<lingxia::network::NetworkInfo> {
    lingxia::network::current(&app).await
}

// ---------------------------------------------------------------------------
// demo.download — async, Arc<LxApp> + payload. Wraps lingxia::file::download.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct DownloadInput {
    url: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadOutput {
    file_name: String,
    size: u64,
    mime_type: Option<String>,
    path: String,
}

#[lingxia::native("demo.download")]
async fn demo_download(
    app: Arc<LxApp>,
    input: DownloadInput,
) -> Result<DownloadOutput> {
    let downloaded =
        lingxia::file::download(&app, lingxia::file::DownloadRequest::new(input.url)).await?;
    Ok(DownloadOutput {
        file_name: downloaded.file_name().to_owned(),
        size: downloaded.size(),
        mime_type: downloaded.mime_type().map(str::to_owned),
        path: downloaded.path().to_string_lossy().into_owned(),
    })
}

// ---------------------------------------------------------------------------
// demo.chooseMedia — async, Arc<LxApp> + payload. Wraps lingxia::media::choose_media.
// The facade returns a serialized JSON string; we re-parse it so the view gets
// a structured object.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChooseMediaInput {
    #[serde(default = "default_max_count")]
    max_count: u32,
    /// "images" | "videos" | "mix" (defaults to images).
    #[serde(default)]
    mode: Option<String>,
}

fn default_max_count() -> u32 {
    9
}

#[lingxia::native("demo.chooseMedia")]
async fn demo_choose_media(
    app: Arc<LxApp>,
    input: ChooseMediaInput,
) -> Result<serde_json::Value> {
    let mode = match input.mode.as_deref() {
        Some("videos") => lingxia::media::ChooseMediaMode::Videos,
        Some("mix") => lingxia::media::ChooseMediaMode::Mix,
        _ => lingxia::media::ChooseMediaMode::Images,
    };
    let request = lingxia::media::ChooseMediaRequest {
        max_count: input.max_count,
        mode,
        ..Default::default()
    };
    let raw = lingxia::media::choose_media(&app, request).await?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or(serde_json::Value::String(raw));
    Ok(value)
}

// ---------------------------------------------------------------------------
// demo.ticker — stream. Pushes a few Tick events to the page, then ends with a
// summary. Demonstrates a Rust → page push.
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Tick {
    index: u32,
    at_ms: u128,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TickerSummary {
    emitted: u32,
}

#[lingxia::native("demo.ticker", stream)]
async fn demo_ticker(mut stream: StreamContext<Tick, TickerSummary>) -> HostResult<()> {
    let count: u32 = 5;
    let start = std::time::Instant::now();
    for index in 0..count {
        if stream.canceled().await {
            return Ok(());
        }
        stream.send(Tick {
            index,
            at_ms: start.elapsed().as_millis(),
        })?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    stream.end(TickerSummary { emitted: count })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// demo.echo — channel. Echoes each received message back, demonstrating a
// bidirectional View <-> Logic stream.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct EchoIn {
    text: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EchoOut {
    echo: String,
    seq: u64,
}

#[lingxia::native("demo.echo", channel)]
async fn demo_echo(mut channel: ChannelContext<EchoIn, EchoOut>) -> HostResult<()> {
    let mut seq: u64 = 0;
    while let Some(message) = channel.recv().await? {
        match message {
            ChannelMessage::Data(data) => {
                seq += 1;
                channel.send(EchoOut {
                    echo: data.text,
                    seq,
                })?;
            }
            ChannelMessage::Close { .. } => break,
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Host addon: install all routes via the build.rs-generated registry.
// ---------------------------------------------------------------------------

// Auto-generated by build.rs; defines `__lingxia_native::install()`.
include!(concat!(env!("OUT_DIR"), "/lingxia_native_handlers.rs"));

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_host_apis(&self) {
        // Registers every #[lingxia::native] route discovered at build time.
        __lingxia_native::install();
    }

    fn start_services(&self) {
        // No background services in this example.
    }
}

fn register_host_addons() {
    lingxia::register_host_addon(Box::new(AppHostAddon));
}

// iOS/macOS: C export. The Apple SDK calls this symbol at startup.
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    register_host_addons();
}
