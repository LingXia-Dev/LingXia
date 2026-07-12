use crate::i18n::js_error_from_platform_error;
use crate::i18n::js_internal_error;
use lingxia_messaging::{CallbackResult, register_handler, remove_callback};
use lingxia_platform::traits::network::Network;
use lxapp::{
    LxApp, info, lx, publish_app_event, register_app_handler, unregister_app_handler, warn,
};
use rong::function::Optional;
use rong::{IntoJSObject, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, Ipv6Addr};

const NETWORK_CHANGE_EVENT: &str = "NetworkChange";

#[derive(Clone, Copy, Default)]
struct NetworkCallbackId(Option<u64>);

fn set_network_callback_id(ctx: &JSContext, id: Option<u64>) {
    ctx.set_state(NetworkCallbackId(id));
}

fn network_callback_id(ctx: &JSContext) -> Option<u64> {
    ctx.get_state::<NetworkCallbackId>().and_then(|s| s.0)
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSNetworkInfoResult {
    #[js_name = "isConnected"]
    is_connected: bool,
    #[js_name = "networkType"]
    network_type: String,
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

fn normalize_network_type(raw: Option<&str>) -> &'static str {
    let Some(value) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return "unknown";
    };
    match value.to_ascii_lowercase().as_str() {
        "wifi" => "wifi",
        "2g" => "2g",
        "3g" => "3g",
        "4g" => "4g",
        "5g" => "5g",
        "ethernet" => "ethernet",
        "none" => "none",
        // Legacy value from older native implementations.
        "cellular" | "unknown" => "unknown",
        _ => "unknown",
    }
}

fn parse_json_payload(data: &str, label: &str) -> Result<Value, RongJSError> {
    serde_json::from_str(data)
        .map_err(|e| js_internal_error(format!("Failed to parse {}: {}", label, e)))
}

fn parse_string_array(parsed: &Value, key: &str) -> Vec<String> {
    parsed
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_primary_ipv4(values: Vec<String>) -> Vec<String> {
    let mut normalized = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if let Ok(ip) = trimmed.parse::<Ipv4Addr>()
            && !ip.is_loopback()
            && !ip.is_unspecified()
        {
            normalized.insert(ip.to_string());
        }
    }
    normalized.into_iter().take(1).collect()
}

fn normalize_primary_ipv6(values: Vec<String>) -> Vec<String> {
    let mut normalized = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if let Ok(ip) = trimmed.parse::<Ipv6Addr>()
            && !ip.is_loopback()
            && !ip.is_unspecified()
            && !ip.is_multicast()
            && !ip.is_unicast_link_local()
        {
            normalized.insert(ip.to_string());
        }
    }
    normalized.into_iter().take(1).collect()
}

fn normalize_network_info(parsed: &Value) -> JSNetworkInfoResult {
    let mut network_type =
        normalize_network_type(parsed.get("networkType").and_then(Value::as_str)).to_string();
    let is_connected = parsed
        .get("isConnected")
        .and_then(Value::as_bool)
        .unwrap_or(network_type != "none");
    let mut ipv4 = normalize_primary_ipv4(parse_string_array(parsed, "ipv4"));
    let mut ipv6 = normalize_primary_ipv6(parse_string_array(parsed, "ipv6"));

    if !is_connected {
        network_type = "none".to_string();
        ipv4.clear();
        ipv6.clear();
    }

    JSNetworkInfoResult {
        is_connected,
        network_type,
        ipv4,
        ipv6,
    }
}

fn network_info_to_json(info: &JSNetworkInfoResult) -> String {
    json!({
        "isConnected": info.is_connected,
        "networkType": info.network_type,
        "ipv4": info.ipv4,
        "ipv6": info.ipv6,
    })
    .to_string()
}

fn normalize_network_change_payload(payload: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(payload).ok()?;
    let info = normalize_network_info(&parsed);
    Some(network_info_to_json(&info))
}

fn parse_network_info(data: String) -> Result<JSNetworkInfoResult, RongJSError> {
    let parsed = parse_json_payload(&data, "network info")?;
    Ok(normalize_network_info(&parsed))
}

async fn get_network_info(ctx: JSContext) -> JSResult<JSNetworkInfoResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let data = lxapp
        .runtime
        .get_network_info()
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    parse_network_info(data)
}

fn ensure_network_change_callback(ctx: &JSContext) -> JSResult<()> {
    if network_callback_id(ctx).is_some() {
        return Ok(());
    }

    let lxapp = LxApp::from_ctx(ctx)?;
    let appid = lxapp.appid.clone();
    let appid_for_cb = appid.clone();

    let callback_id = register_handler(move |result| {
        if let CallbackResult::Success(payload) = result {
            let payload_json = normalize_network_change_payload(&payload);
            let emitted = publish_app_event(&appid_for_cb, NETWORK_CHANGE_EVENT, payload_json);
            if !emitted {
                warn!(
                    "NetworkChange publish_app_event failed appid={}",
                    appid_for_cb
                );
            }
        }
    });

    if let Err(err) = lxapp.runtime.add_network_change_listener(callback_id) {
        remove_callback(callback_id);
        return Err(js_error_from_platform_error(&err));
    }

    info!(
        "NetworkChange callback registered appid={} callback_id={}",
        appid, callback_id
    );
    set_network_callback_id(ctx, Some(callback_id));
    Ok(())
}

fn clear_network_change_callback(ctx: &JSContext) -> JSResult<()> {
    let Some(callback_id) = network_callback_id(ctx) else {
        return Ok(());
    };

    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .runtime
        .remove_network_change_listener(callback_id)
        .map_err(|err| js_error_from_platform_error(&err))?;
    remove_callback(callback_id);
    set_network_callback_id(ctx, None);
    Ok(())
}

fn on_network_change(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    register_app_handler(&ctx, NETWORK_CHANGE_EVENT, callback.clone())?;
    if let Err(err) = ensure_network_change_callback(&ctx) {
        let _ = unregister_app_handler(&ctx, NETWORK_CHANGE_EVENT, Some(callback));
        return Err(err);
    }
    Ok(())
}

fn off_network_change(ctx: JSContext, callback: Optional<JSFunc>) -> JSResult<()> {
    let remaining = unregister_app_handler(&ctx, NETWORK_CHANGE_EVENT, callback.0);
    if remaining == 0 {
        clear_network_change_callback(&ctx)?;
    }
    Ok(())
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_network_info_func = JSFunc::new(ctx, get_network_info)?;
    lx::register_js_api(ctx, "getNetworkInfo", get_network_info_func)?;

    let on_network_change_func = JSFunc::new(ctx, on_network_change)?;
    lx::register_js_api(ctx, "onNetworkChange", on_network_change_func)?;

    let off_network_change_func = JSFunc::new(ctx, off_network_change)?;
    lx::register_js_api(ctx, "offNetworkChange", off_network_change_func)?;

    Ok(())
}
