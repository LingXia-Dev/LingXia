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
