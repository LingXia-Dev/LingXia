use std::sync::Arc;

#[derive(serde::Deserialize)]
struct EchoInput {
    value: String,
}

#[derive(serde::Serialize)]
struct EchoOutput {
    value: String,
}

#[lingxia::native("facade.echo")]
fn facade_echo(_app: Arc<lingxia::LxApp>, input: EchoInput) -> lingxia::Result<EchoOutput> {
    Ok(EchoOutput { value: input.value })
}

#[lingxia::native("facade.blockingEcho", blocking)]
fn facade_blocking_echo(input: EchoInput) -> lingxia::Result<EchoOutput> {
    Ok(EchoOutput { value: input.value })
}

#[derive(serde::Serialize)]
struct StreamEvent {
    value: u32,
}

#[lingxia::native("facade.stream", stream)]
async fn facade_stream(
    mut stream: lingxia::host::StreamContext<StreamEvent, String>,
) -> lingxia::Result<()> {
    stream.send(StreamEvent { value: 1 })?;
    stream.end("done".to_string())?;
    Ok(())
}

#[derive(serde::Deserialize)]
struct ChannelIn {
    value: String,
}

#[derive(serde::Serialize)]
struct ChannelOut {
    value: String,
}

#[lingxia::native("facade.channel", channel)]
async fn facade_channel(
    mut channel: lingxia::host::ChannelContext<ChannelIn, ChannelOut>,
) -> lingxia::Result<()> {
    while let Some(message) = channel.recv().await? {
        if let lingxia::host::ChannelMessage::Data(input) = message {
            channel.send(ChannelOut { value: input.value })?;
        }
    }
    Ok(())
}

#[test]
fn native_macro_accepts_lingxia_result_handlers() {
    let _ = facade_echo_host();
    let _ = facade_blocking_echo_host();
    let _ = facade_stream_host();
    let _ = facade_channel_host();
}

#[test]
fn root_js_extension_exports_stay_scoped_to_js_module() {
    let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lingxia lib.rs");
    let root_exports = lib
        .split("pub mod log {")
        .next()
        .expect("root exports section");
    assert!(
        !root_exports.contains("pub use lxapp::lx::{LxLogicExtension, register_logic_extension}")
    );
    assert!(!root_exports.contains("pub use lingxia_log::{"));
    assert!(!root_exports.contains("pub use lingxia_update::{"));
    assert!(!root_exports.contains("pub use lingxia_provider::{"));
    assert!(!root_exports.contains("pub use lingxia_media::{"));
    assert!(!root_exports.contains("pub use tokio;"));
    assert!(!root_exports.contains("pub use lxapp::set_num_workers"));
    assert!(!root_exports.contains("create_page_instance"));
    assert!(!root_exports.contains("PageInstance"));
    assert!(!root_exports.contains("PageOwner"));
    assert!(!root_exports.contains("PageTarget"));
    assert!(!root_exports.contains("pub mod browser;"));
    assert!(!root_exports.contains("pub mod downloads;"));
    assert!(!root_exports.contains("pub mod settings;"));
    assert!(!root_exports.contains("app_config"));
    assert!(!root_exports.contains("AppConfig"));
}

#[test]
fn windows_facade_must_not_reexport_webview_internals() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/windows.rs"))
        .expect("read lingxia src/windows.rs");
    assert!(
        !source.contains("pub use lingxia_webview::"),
        "src/windows.rs must not re-export lingxia_webview internals"
    );
}

#[test]
fn file_download_facade_stays_user_cache_scoped() {
    let file = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/file.rs"))
        .expect("read lingxia file.rs");

    assert!(!file.contains("pub fn to_path("));
    assert!(!file.contains("download_to_path_with_behavior"));
    assert!(file.contains("download_to_user_cache"));
}

#[test]
fn log_facade_exports_app_authoring_surface_only() {
    let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lingxia lib.rs");
    let log_exports = lib
        .split("pub mod log {")
        .nth(1)
        .and_then(|rest| rest.split("/// Android platform bridge exports").next())
        .expect("log exports section");

    assert!(log_exports.contains("register_downstream_logger"));
    assert!(log_exports.contains("attach_log_stream"));
    assert!(!log_exports.contains("LogManager"));
    assert!(!log_exports.contains("LogBuffer"));
    assert!(!log_exports.contains("register_log_provider"));
    assert!(!log_exports.contains("tracing_layer"));
    assert!(!log_exports.contains("upload_collected_logs"));
    assert!(!log_exports.contains("CollectedLogArchive"));
}

#[test]
fn update_facade_exposes_host_app_module_only() {
    let update = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/update.rs"))
        .expect("read lingxia update.rs");

    assert!(!update.contains("pub use lingxia_service::update"));
    assert!(!update.contains("pub use lingxia_update"));
    assert!(!update.contains("pub fn configure"));
    assert!(update.contains("pub mod host_app"));
    assert!(update.contains("pub fn set_installer"));
    assert!(update.contains("pub fn on_progress"));
    assert!(update.contains("pub async fn check()"));
    assert!(update.contains("pub enum Outcome"));
    assert!(update.contains("pub enum Progress"));
}

#[test]
fn lingxia_logic_must_not_depend_on_lingxia() {
    let manifest = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lingxia-logic/Cargo.toml"
    ))
    .expect("read lingxia-logic manifest");
    assert!(
        !manifest
            .lines()
            .any(|line| line.trim_start().starts_with("lingxia ="))
    );
}

#[test]
fn native_channel_macro_closes_on_handler_error() {
    let macro_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lingxia-native-macros/src/lib.rs"
    ))
    .expect("read lingxia-native-macros lib.rs");
    assert!(macro_src.contains("let __lingxia_close = __lingxia_ctx.close_handle();"));
    assert!(macro_src.contains("__lingxia_close.close_with(\"HOST_ERROR\", err.to_string())"));
}
