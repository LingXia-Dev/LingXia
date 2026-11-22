use crate::i18n::js_internal_error;
use lingxia_messaging::{CallbackResult, register_handler, remove_callback};
use lingxia_platform::Platform;
use lingxia_platform::traits::network::Network;
use lxapp::{
    LxApp, info, publish_app_event, register_app_handler, unregister_app_handler_token, warn,
};
use rong::{
    IntoJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, function::Optional,
};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const NETWORK_CHANGE_EVENT: &str = "NetworkChange";

/// The latest connectivity the platform reported to this Logic context, kept
/// so `lx.network.status.get()` answers without a native round trip.
struct NetworkState {
    latest: Arc<Mutex<Option<JSNetworkInfoResult>>>,
    _listener: Option<NetworkListener>,
}

/// The platform connectivity listener, removed with the context that owns it.
struct NetworkListener {
    callback_id: u64,
    runtime: Arc<Platform>,
}

impl Drop for NetworkListener {
    fn drop(&mut self) {
        let _ = self
            .runtime
            .remove_network_change_listener(self.callback_id);
        remove_callback(self.callback_id);
    }
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
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

fn parse_network_info(data: String) -> Result<JSNetworkInfoResult, RongJSError> {
    let parsed = parse_json_payload(&data, "network info")?;
    Ok(normalize_network_info(&parsed))
}

/// What `get()` answers before the platform's first report: not known to be
/// connected, and `'unknown'` rather than `'none'`, which means offline.
fn not_yet_reported() -> JSNetworkInfoResult {
    JSNetworkInfoResult {
        is_connected: false,
        network_type: "unknown".to_string(),
        ipv4: Vec::new(),
        ipv6: Vec::new(),
    }
}

fn latest_network_info(ctx: &JSContext) -> JSNetworkInfoResult {
    ctx.get_state::<NetworkState>()
        .and_then(|state| state.latest.lock().ok().and_then(|latest| latest.clone()))
        .unwrap_or_else(not_yet_reported)
}

/// Subscribes this context to the platform's connectivity reports once, and
/// seeds the cache with one read. A host without the listener leaves `get()`
/// at its not-yet-reported value instead of failing Logic startup.
fn start_network_cache(ctx: &JSContext) {
    let latest = Arc::new(Mutex::new(None));
    let listener = match LxApp::from_ctx(ctx) {
        Ok(lxapp) => {
            let listener = add_listener(&lxapp, latest.clone());
            seed(ctx, &lxapp, latest.clone());
            listener
        }
        Err(err) => {
            warn!("lx.network.status has no lxapp to follow: {err}");
            None
        }
    };
    ctx.set_state(NetworkState {
        latest,
        _listener: listener,
    });
}

fn add_listener(
    lxapp: &Arc<LxApp>,
    latest: Arc<Mutex<Option<JSNetworkInfoResult>>>,
) -> Option<NetworkListener> {
    let appid = lxapp.appid.clone();
    let callback_id = register_handler(move |result| {
        let CallbackResult::Success(payload) = result else {
            return;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&payload) else {
            return;
        };
        let info = normalize_network_info(&parsed);
        let payload_json = network_info_to_json(&info);
        if let Ok(mut latest) = latest.lock() {
            *latest = Some(info);
        }
        if !publish_app_event(&appid, NETWORK_CHANGE_EVENT, Some(payload_json)) {
            warn!("NetworkChange publish_app_event failed appid={}", appid);
        }
    });
    if let Err(err) = lxapp.runtime.add_network_change_listener(callback_id) {
        remove_callback(callback_id);
        warn!("lx.network.status cannot follow connectivity: {err}");
        return None;
    }
    info!(
        "NetworkChange callback registered appid={} callback_id={}",
        lxapp.appid, callback_id
    );
    Some(NetworkListener {
        callback_id,
        runtime: lxapp.runtime.clone(),
    })
}

/// One read for the value before the first change; dropped if a report beat it.
fn seed(ctx: &JSContext, lxapp: &Arc<LxApp>, latest: Arc<Mutex<Option<JSNetworkInfoResult>>>) {
    let lxapp = lxapp.clone();
    ctx.spawn_task(async move {
        let info = match lxapp.runtime.get_network_info().await {
            Ok(data) => match parse_network_info(data) {
                Ok(info) => info,
                Err(err) => {
                    warn!("lx.network.status seed read was malformed: {err}");
                    return;
                }
            },
            Err(err) => {
                warn!("lx.network.status could not read the current status: {err}");
                return;
            }
        };
        let payload_json = network_info_to_json(&info);
        {
            let Ok(mut latest) = latest.lock() else {
                return;
            };
            if latest.is_some() {
                return;
            }
            *latest = Some(info);
        }
        publish_app_event(&lxapp.appid, NETWORK_CHANGE_EVENT, Some(payload_json));
    });
}

/// The connectivity last reported: whether it is up, its type, and addresses.
/// No native round trip; before the first report `networkType` is `'unknown'`.
fn get_network_status(ctx: JSContext) -> JSResult<JSNetworkInfoResult> {
    Ok(latest_network_info(&ctx))
}

fn json_of(value: &JSValue) -> Option<Value> {
    let object = value.clone().to_rust::<JSObject>().ok()?;
    serde_json::from_str(&object.to_json_string().ok()?).ok()
}

/// Delivers the current connectivity before returning, then each change.
fn watch_network_status(
    ctx: JSContext,
    callback: JSFunc,
    options: Optional<JSValue>,
) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let current = latest_network_info(&ctx);
        let delivered: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(
            serde_json::from_str(&network_info_to_json(&current)).ok(),
        ));
        let on_change = {
            let delivered = delivered.clone();
            let callback = callback.clone();
            JSFunc::new(&ctx, move |info: JSValue| {
                let next = json_of(&info);
                if next.is_some() && *delivered.borrow() == next {
                    return;
                }
                *delivered.borrow_mut() = next;
                let _ = callback.call::<_, ()>(None, (info,));
            })?
        };
        let token = register_app_handler(&ctx, NETWORK_CHANGE_EVENT, on_change)?;
        let _ = callback.call::<_, ()>(None, (current,));

        let off_ctx = ctx.clone();
        let unsubscribed = Cell::new(false);
        JSFunc::new(&ctx, move || {
            if unsubscribed.replace(true) {
                return;
            }
            unregister_app_handler_token(&off_ctx, NETWORK_CHANGE_EVENT, token);
        })
    })
}

fn status_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let status = JSObject::new(ctx);
    status.set("get", JSFunc::new(ctx, get_network_status)?.name("get")?)?;
    status.set(
        "watch",
        JSFunc::new(ctx, watch_network_status)?.name("watch")?,
    )?;
    Ok(status)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)?;
    register_network_api(ctx)?;
    start_network_cache(ctx);
    Ok(())
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const network: "NetworkApi" = crate::namespace::network(ctx)?;
    }
}

rong::js_api! {
    fn register_network_api(ctx) {
        namespace NetworkApi = crate::namespace::network(ctx)?;
        const wifi: "WifiApi" = crate::namespace::wifi(ctx)?;
        const status: "Live<NetworkInfo>" = status_namespace(ctx)?;
    }
}
