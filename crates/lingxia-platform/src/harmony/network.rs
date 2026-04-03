use crate::error::PlatformError;
use crate::traits::network::Network;
use lingxia_webview::platform::harmony::tsfn::call_arkts;

use super::Platform;

const CALLBACK_ERR_INTERNAL: u32 = 1000;

fn call_network_stream(method: &str, callback_id: u64) -> Result<(), PlatformError> {
    let callback_id_str = callback_id.to_string();
    call_arkts(method, &[callback_id_str.as_str()]).map_err(|e| {
        let _ = lingxia_messaging::invoke_callback(callback_id, Err(CALLBACK_ERR_INTERNAL));
        PlatformError::Platform(format!("Failed to call {}: {}", method, e))
    })
}

impl Network for Platform {
    async fn get_network_info(&self) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| {
            let callback_id_str = callback_id.to_string();
            call_arkts("getNetworkInfo", &[callback_id_str.as_str()]).map_err(|e| {
                PlatformError::Platform(format!("Failed to call getNetworkInfo: {}", e))
            })
        })
        .await
    }

    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        // Listener is callback-based; the ArkTS side will push updates via `onCallback`.
        call_network_stream("addNetworkChangeListener", callback_id)
    }

    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_network_stream("removeNetworkChangeListener", callback_id)
    }
}
