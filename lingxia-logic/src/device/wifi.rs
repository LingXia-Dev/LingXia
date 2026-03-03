use crate::i18n::{js_error_from_business_code, js_error_from_platform_error};
use crate::i18n::{js_internal_error, js_timeout_error};
use lingxia_messaging::{CallbackResult, get_callback, register_handler, remove_callback};
use lingxia_platform::traits::wifi::{Wifi, WifiConnectRequest, WifiGetConnectedRequest};
use lxapp::{LxApp, lx, publish_app_event, register_app_handler, unregister_app_handler};
use lxapp::{info, warn};
use rong::function::Optional;
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::{Value, json};

const WIFI_CONNECTED_EVENT: &str = "WifiConnected";

#[derive(Clone, Copy, Default)]
struct WifiCallbackId(Option<u64>);

fn set_wifi_callback_id(ctx: &JSContext, id: Option<u64>) {
    ctx.set_state(WifiCallbackId(id));
}

fn wifi_callback_id(ctx: &JSContext) -> Option<u64> {
    ctx.get_state::<WifiCallbackId>().and_then(|s| s.0)
}

fn normalize_wifi_connected_payload(payload: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(payload).ok()?;

    // Extract with unified field names (support both "ssid"/"SSID" variants)
    let ssid = parsed
        .get("ssid")
        .or_else(|| parsed.get("SSID"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let bssid = parsed
        .get("bssid")
        .or_else(|| parsed.get("BSSID"))
        .and_then(Value::as_str);
    let connected = parsed
        .get("connected")
        .and_then(Value::as_bool)
        .unwrap_or(!ssid.is_empty());
    let secure = parsed
        .get("secure")
        .and_then(Value::as_bool)
        .unwrap_or(connected);
    let signal_strength = parsed
        .get("signalStrength")
        .and_then(Value::as_u64)
        .map(|v| v.min(100) as u8)
        .unwrap_or(if connected { 100 } else { 0 });
    let state = parsed
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or(if connected {
            "connected"
        } else {
            "disconnected"
        });
    let frequency = parsed
        .get("frequency")
        .and_then(Value::as_u64)
        .map(|v| v as u32);

    // Build normalized payload
    let mut result = json!({
        "SSID": ssid,
        "secure": secure,
        "signalStrength": signal_strength,
        "connected": connected,
        "state": state,
    });

    if let Some(bssid) = bssid {
        result["BSSID"] = Value::String(bssid.to_string());
    }
    if let Some(freq) = frequency {
        result["frequency"] = Value::from(freq);
    }

    Some(result.to_string())
}

fn ensure_wifi_connected_callback(ctx: &JSContext) -> JSResult<()> {
    if wifi_callback_id(ctx).is_some() {
        return Ok(());
    }

    let lxapp = LxApp::from_ctx(ctx)?;
    let appid = lxapp.appid.clone();
    let appid_for_cb = appid.clone();
    let callback_id = register_handler(move |result| {
        if let CallbackResult::Success(payload) = result {
            info!("WifiConnected native callback received: {}", payload);
            let payload_json = normalize_wifi_connected_payload(&payload);
            let emitted = publish_app_event(&appid_for_cb, WIFI_CONNECTED_EVENT, payload_json);
            if !emitted {
                warn!(
                    "WifiConnected publish_app_event failed appid={}",
                    appid_for_cb
                );
            }
        }
    });

    if let Err(err) = lxapp.runtime.add_wifi_state_listener(callback_id) {
        remove_callback(callback_id);
        return Err(js_error_from_platform_error(&err));
    }

    info!(
        "WifiConnected callback registered appid={} callback_id={}",
        appid, callback_id
    );
    set_wifi_callback_id(ctx, Some(callback_id));
    Ok(())
}

fn clear_wifi_connected_callback(ctx: &JSContext) -> JSResult<()> {
    let Some(callback_id) = wifi_callback_id(ctx) else {
        return Ok(());
    };

    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .runtime
        .remove_wifi_state_listener(callback_id)
        .map_err(|err| js_error_from_platform_error(&err))?;
    remove_callback(callback_id);
    info!(
        "WifiConnected callback cleared appid={} callback_id={}",
        lxapp.appid, callback_id
    );
    set_wifi_callback_id(ctx, None);
    Ok(())
}

/// WiFi information from JavaScript
#[derive(Debug, Clone, IntoJSObj)]
pub struct JSWifiInfo {
    /// Service Set Identifier (network name)
    #[rename = "SSID"]
    ssid: String,
    /// Basic Service Set Identifier (MAC address)
    #[rename = "BSSID"]
    bssid: Option<String>,
    /// Whether the network is secure (requires password)
    secure: bool,
    /// Signal strength (0-100, higher is better)
    #[rename = "signalStrength"]
    signal_strength: u8,
    /// Center frequency in MHz (if available)
    #[rename = "frequency"]
    frequency: Option<u32>,
}

/// Parse WiFi info from a JSON value
fn parse_wifi_info_from_json(item: &Value, default_signal: u8, default_secure: bool) -> JSWifiInfo {
    let signal_strength = item
        .get("signalStrength")
        .and_then(Value::as_u64)
        .map(|v| v.min(100) as u8) // Clamp to valid range to prevent overflow
        .unwrap_or(default_signal);

    JSWifiInfo {
        ssid: item
            .get("ssid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        bssid: item.get("bssid").and_then(Value::as_str).map(String::from),
        secure: item
            .get("secure")
            .and_then(Value::as_bool)
            .unwrap_or(default_secure),
        signal_strength,
        frequency: item
            .get("frequency")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
    }
}

/// WiFi connection options from JavaScript
#[derive(FromJSObj)]
struct JSConnectWifiOptions {
    /// SSID of the network to connect to
    #[rename = "SSID"]
    ssid: String,
    /// Network password (omit for open networks)
    password: Option<String>,
}

/// Generic handler for WiFi callbacks that return no data
async fn handle_wifi_callback<F>(
    ctx: &JSContext,
    platform_call: F,
    operation_name: &str,
) -> JSResult<()>
where
    F: FnOnce(u64) -> Result<(), lingxia_platform::error::PlatformError>,
{
    let _lxapp = LxApp::from_ctx(ctx)?;
    let (callback_id, receiver) = get_callback();

    platform_call(callback_id).map_err(|e| js_error_from_platform_error(&e))?;

    match receiver.await {
        Ok(CallbackResult::Success(_)) => Ok(()),
        Ok(CallbackResult::Error(code)) => Err(js_error_from_business_code(code)),
        Err(_) => Err(js_timeout_error(format!(
            "{} callback timed out",
            operation_name
        ))),
    }
}

/// Generic handler for WiFi callbacks that return parsed data
async fn handle_wifi_callback_with_data<T, F, P>(
    ctx: &JSContext,
    platform_call: F,
    parser: P,
    operation_name: &str,
) -> JSResult<T>
where
    F: FnOnce(u64) -> Result<(), lingxia_platform::error::PlatformError>,
    P: FnOnce(String) -> Result<T, RongJSError>,
{
    let _lxapp = LxApp::from_ctx(ctx)?;
    let (callback_id, receiver) = get_callback();

    platform_call(callback_id).map_err(|e| js_error_from_platform_error(&e))?;

    match receiver.await {
        Ok(CallbackResult::Success(data)) => parser(data),
        Ok(CallbackResult::Error(code)) => Err(js_error_from_business_code(code)),
        Err(_) => Err(js_timeout_error(format!(
            "{} callback timed out",
            operation_name
        ))),
    }
}

/// Parse WiFi list from callback result
fn parse_wifi_list(data: String) -> Result<Vec<JSWifiInfo>, RongJSError> {
    let parsed: Value = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("Failed to parse WiFi list: {}", e)))?;

    let wifi_array = parsed
        .as_array()
        .ok_or_else(|| js_internal_error("WiFi list is not an array"))?;

    let wifi_list = wifi_array
        .iter()
        .map(|item| parse_wifi_info_from_json(item, 0, false))
        .collect();

    Ok(wifi_list)
}

/// Parse connected WiFi info from callback result
fn parse_connected_wifi(data: String) -> Result<JSWifiInfo, RongJSError> {
    let parsed: Value = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("Failed to parse WiFi info: {}", e)))?;

    Ok(parse_wifi_info_from_json(&parsed, 100, true))
}

/// Initialize WiFi module
async fn start_wifi(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Check if WiFi is enabled before attempting to start
    let wifi_enabled = lxapp
        .runtime
        .is_wifi_enabled()
        .map_err(|e| js_error_from_platform_error(&e))?;

    if !wifi_enabled {
        return Err(js_error_from_business_code(12009));
    }

    handle_wifi_callback(&ctx, |id| lxapp.runtime.start_wifi(id), "start WiFi").await
}

/// Stop WiFi module
async fn stop_wifi(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    handle_wifi_callback(&ctx, |id| lxapp.runtime.stop_wifi(id), "stop WiFi").await
}

/// Connect to WiFi (async - waits for request submission, not actual connection)
///
/// This function returns when the connection request is successfully submitted to the system.
/// The actual connection status will be reported via onWifiConnected event.
async fn connect_wifi(ctx: JSContext, options: JSConnectWifiOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let JSConnectWifiOptions { ssid, password } = options;
    handle_wifi_callback(
        &ctx,
        move |id| {
            let request = WifiConnectRequest {
                callback_id: id,
                ssid,
                password,
            };
            lxapp.runtime.connect_wifi(request)
        },
        "connect WiFi",
    )
    .await
}

/// Get WiFi list (scan results)
async fn get_wifi_list(ctx: JSContext) -> JSResult<Vec<JSWifiInfo>> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    handle_wifi_callback_with_data(
        &ctx,
        |id| lxapp.runtime.get_wifi_list(id),
        parse_wifi_list,
        "get WiFi list",
    )
    .await
}

/// Get connected WiFi info
async fn get_connected_wifi(ctx: JSContext) -> JSResult<JSWifiInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    handle_wifi_callback_with_data(
        &ctx,
        |id| {
            let request = WifiGetConnectedRequest { callback_id: id };
            lxapp.runtime.get_connected_wifi(request)
        },
        parse_connected_wifi,
        "get connected WiFi",
    )
    .await
}

fn on_wifi_connected(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    info!("onWifiConnected called");
    ensure_wifi_connected_callback(&ctx)?;
    register_app_handler(&ctx, WIFI_CONNECTED_EVENT, callback)?;
    Ok(())
}

fn off_wifi_connected(ctx: JSContext, callback: Optional<JSFunc>) -> JSResult<()> {
    info!("offWifiConnected called");
    let remaining = unregister_app_handler(&ctx, WIFI_CONNECTED_EVENT, callback.0);
    if remaining == 0 {
        clear_wifi_connected_callback(&ctx)?;
    }
    Ok(())
}

/// Initialize WiFi API bindings
pub fn init(ctx: &JSContext) -> JSResult<()> {
    let start_wifi_func = JSFunc::new(ctx, start_wifi)?;
    lx::register_js_api(ctx, "startWifi", start_wifi_func)?;

    let stop_wifi_func = JSFunc::new(ctx, stop_wifi)?;
    lx::register_js_api(ctx, "stopWifi", stop_wifi_func)?;

    let connect_wifi_func = JSFunc::new(ctx, connect_wifi)?;
    lx::register_js_api(ctx, "connectWifi", connect_wifi_func)?;

    let get_wifi_list_func = JSFunc::new(ctx, get_wifi_list)?;
    lx::register_js_api(ctx, "getWifiList", get_wifi_list_func)?;

    let get_connected_wifi_func = JSFunc::new(ctx, get_connected_wifi)?;
    lx::register_js_api(ctx, "getConnectedWifi", get_connected_wifi_func)?;

    let on_wifi_connected_func = JSFunc::new(ctx, on_wifi_connected)?;
    lx::register_js_api(ctx, "onWifiConnected", on_wifi_connected_func)?;

    let off_wifi_connected_func = JSFunc::new(ctx, off_wifi_connected)?;
    lx::register_js_api(ctx, "offWifiConnected", off_wifi_connected_func)?;

    Ok(())
}
