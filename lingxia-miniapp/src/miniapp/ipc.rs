// IPC script content included at compile time
const IPC_SCRIPT: &str = include_str!("scripts/ipc.js");

/// Returns the IPC script content that should be injected into WebView
pub fn get_ipc_script() -> &'static str {
    IPC_SCRIPT
}

