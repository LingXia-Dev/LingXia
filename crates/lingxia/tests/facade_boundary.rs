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

#[lingxia::native("facade.syncEcho", sync)]
fn facade_sync_echo(input: EchoInput) -> lingxia::Result<EchoOutput> {
    Ok(EchoOutput { value: input.value })
}

#[lingxia::native("facade.scope")]
fn facade_scope(context: lingxia::host::HostInvocationContext) -> lingxia::Result<()> {
    let _ = context.caller();
    let scope = context.app_scope();
    let _ = scope.identity().app_id();
    let _ = scope.identity().session_id();
    let _ = scope.storage().user_data();
    let _ = scope.resource_grants();
    let _ = context.lxapp();
    Ok(())
}

#[derive(serde::Serialize)]
struct StreamEvent {
    value: u32,
}

#[lingxia::native("facade.stream", stream)]
async fn facade_stream(progress: lingxia::Progress<StreamEvent>) -> lingxia::Result<String> {
    progress.send(StreamEvent { value: 1 })?;
    Ok("done".to_string())
}

#[lingxia::native("facade.scopeStream", stream)]
fn facade_scope_stream(
    context: lingxia::host::HostInvocationContext,
    progress: lingxia::Progress<StreamEvent>,
    input: EchoInput,
) -> lingxia::Result<String> {
    if progress.is_canceled() {
        return Err(lingxia::Error::aborted("Canceled"));
    }
    Ok(format!(
        "{}:{}",
        context.app_scope().identity().app_id(),
        input.value
    ))
}

#[lingxia::native("facade.watch", watch)]
fn facade_watch(
    input: EchoInput,
) -> lingxia::Result<impl futures::Stream<Item = StreamEvent> + Send + 'static> {
    let value = input.value.len() as u32;
    Ok(futures::stream::iter([StreamEvent { value }]))
}

#[lingxia::native("facade.asyncWatch", watch)]
async fn facade_async_watch(
    _app: Arc<lingxia::LxApp>,
) -> Result<impl futures::Stream<Item = StreamEvent> + Send + 'static, NotesError> {
    Ok(futures::stream::empty())
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

#[lingxia::native("facade.scopeChannel", channel)]
async fn facade_scope_channel(
    context: lingxia::host::HostInvocationContext,
    mut channel: lingxia::host::ChannelContext<ChannelIn, ChannelOut>,
) -> lingxia::Result<()> {
    let _ = context.caller();
    while let Some(message) = channel.recv().await? {
        if let lingxia::host::ChannelMessage::Data(input) = message {
            channel.send(ChannelOut { value: input.value })?;
        }
    }
    Ok(())
}

#[lingxia::native("facade.control", audience = "control-app-only")]
fn facade_control() -> lingxia::Result<()> {
    Ok(())
}

#[lingxia::framework_native("facade.controlOrSurface", audience = "control-app-or-surface")]
fn facade_control_or_surface() -> lingxia::Result<()> {
    Ok(())
}

#[derive(Debug, thiserror::Error, serde::Serialize, lingxia::NativeError)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
enum NotesError {
    #[error("note {id} not found")]
    NotFound { id: i64 },
    #[error("notes are locked")]
    Locked,
    #[error(transparent)]
    #[serde(skip)]
    #[native_error(internal)]
    Io(#[from] std::io::Error),
}

#[lingxia::native("facade.appError")]
fn facade_app_error(id: i64) -> Result<(), NotesError> {
    Err(NotesError::NotFound { id })
}

#[lingxia::native("facade.anyhowError", stream)]
async fn facade_anyhow_error(_progress: lingxia::Progress<StreamEvent>) -> anyhow::Result<String> {
    anyhow::bail!("disk full")
}

/// The derive's helper attribute must not clash with an imported route macro.
mod imported_native {
    use lingxia::native;

    #[derive(Debug, thiserror::Error, serde::Serialize, lingxia::NativeError)]
    #[serde(tag = "code")]
    pub enum ImportedError {
        #[error("busy")]
        Busy,
        #[error(transparent)]
        #[serde(skip)]
        #[native_error(internal)]
        Io(#[from] std::io::Error),
    }

    #[native("facade.importedError")]
    pub fn facade_imported_error() -> Result<(), ImportedError> {
        Err(ImportedError::Busy)
    }
}

#[test]
fn native_error_sends_its_code_and_data_and_hides_internals() {
    let error = lingxia::Error::from(NotesError::NotFound { id: 7 });
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({ "code": "NOT_FOUND", "message": "note 7 not found", "data": { "id": 7 } })
    );
    assert_eq!(lingxia::Error::from(NotesError::Locked).code(), "LOCKED");
    let io = std::io::Error::other("disk full");
    let internal = lingxia::Error::from(NotesError::from(io));
    assert_eq!(
        (internal.code(), internal.message()),
        ("E_INTERNAL", "disk full")
    );
    assert!(std::error::Error::source(&internal).is_some());
}

#[test]
fn native_macro_accepts_lingxia_result_handlers() {
    let _ = facade_app_error_host();
    let _ = facade_anyhow_error_host();
    let _ = imported_native::facade_imported_error_host();
    assert_eq!(
        lingxia::Error::from(imported_native::ImportedError::Busy).code(),
        "Busy"
    );
    let _ = facade_echo_host();
    let _ = facade_sync_echo_host();
    let _ = facade_scope_host();
    let _ = facade_stream_host();
    let _ = facade_scope_stream_host();
    let _ = facade_watch_host();
    let _ = facade_async_watch_host();
    let _ = facade_channel_host();
    let _ = facade_scope_channel_host();
    let _ = facade_control_host();
    let _ = facade_control_or_surface_host();
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
fn media_playback_contracts_stay_in_the_playback_namespace() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/media.rs"))
        .expect("read lingxia media facade");
    assert!(source.contains("pub mod playback"));
    assert!(source.contains("pub use lingxia_media::playback::{"));
    assert!(!source.contains("pub use lingxia_media::{"));
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
    assert!(!log_exports.contains("collect_logs"));
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
    assert!(macro_src.contains("Err(err) => __lingxia_close.close_with(err),"));
}

#[test]
fn authority_escape_fixture_covers_all_feature_unification_paths() {
    let manifest = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/authority-escape/Cargo.toml"
    ))
    .expect("read authority escape fixture manifest");
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/authority-escape/src/main.rs"
    ))
    .expect("read authority escape fixture");

    assert!(manifest.contains("lxapp/test-utils"));
    for forbidden in [
        "__init_with_native_authority",
        "NativeHostRuntimeToken",
        "NativeControlPlaneAuthority::for_test",
        "NativeControlPlaneAuthority::for_native_runtime",
        "__install_app_resource_grant_resolver",
        "__install_devtools_resource_grant_resolver",
        "AuthenticatedCaller::LxAppSession",
        "add_global_page_script",
        "LxApp::add_page_script",
        "resolve_settings_destination",
        "apple::resolve_settings_destination_for_host",
    ] {
        assert!(
            source.contains(forbidden),
            "fixture must attempt {forbidden}"
        );
    }
}
