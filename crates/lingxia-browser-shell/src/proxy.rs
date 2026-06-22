use crate::host::{HostResult, StreamContext};
use crate::proxy_settings::{
    AutoSwitchRule, DEFAULT_PROXY_SOCKS_PORT, ProxyMode, ProxyRuleAction, ProxySettings,
};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_proxy::rule_list::{fetch_encoded_from_url, validate_source_url};
use lingxia_proxy::{
    FixedRouter, LocalProxy, ProxyRouter, RouteDecision, RuleListRouter, Socks5Credentials,
    UpstreamConfig,
};
use lingxia_webview::runtime as webview_runtime;
use lingxia_webview::{
    ProxyActivation, ProxyApplyReport, ProxyApplyStatus, ProxyConfig, WebViewError,
};
use lxapp::{LxApp, LxAppError};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProxySettingsInput {
    #[serde(default)]
    mode: ProxyMode,
    #[serde(default)]
    socks5_host: String,
    #[serde(default = "default_proxy_port")]
    socks5_port: u16,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    gfwlist_source_url: String,
    #[serde(default)]
    auto_switch_rules: Vec<AutoSwitchRule>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxySettingsResult {
    mode: ProxyMode,
    socks5_host: String,
    socks5_port: u16,
    username: String,
    password: String,
    auto_switch_rules: Vec<AutoSwitchRule>,
    is_active: bool,
    status: String,
    status_message: String,
    local_proxy_addr: Option<String>,
    gfwlist_ready: bool,
    gfwlist_source_url: String,
    gfwlist_updated_at_ms: Option<u64>,
    gfwlist_status: String,
    gfwlist_status_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GfwListCache {
    encoded: String,
    updated_at_ms: u64,
}

#[derive(Clone)]
struct RunningProxy {
    proxy: LocalProxy,
    local_addr: SocketAddr,
}

#[derive(Clone)]
struct ProxyRuntimeSnapshot {
    is_active: bool,
    status: String,
    status_message: String,
    local_proxy_addr: Option<String>,
}

#[derive(Clone)]
struct GfwListMeta {
    ready: bool,
    updated_at_ms: Option<u64>,
    status: String,
    status_message: String,
}

#[derive(Default)]
struct ProxyRuntimeState {
    running: Option<RunningProxy>,
    snapshot: Option<ProxyRuntimeSnapshot>,
}

struct AutoSwitchRouter {
    rules: Vec<AutoSwitchRule>,
    upstream: UpstreamConfig,
    rule_list_router: Option<RuleListRouter>,
}

fn mode_label(mode: ProxyMode) -> &'static str {
    match mode {
        ProxyMode::Direct => "direct",
        ProxyMode::Global => "global",
        ProxyMode::GfwList => "gfw_list",
    }
}

fn default_proxy_port() -> u16 {
    DEFAULT_PROXY_SOCKS_PORT
}

fn host_matches_rule(host: &str, pattern: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let pattern = pattern.trim().trim_start_matches('.').to_ascii_lowercase();
    if pattern.is_empty() {
        return false;
    }
    host == pattern || host.ends_with(&format!(".{pattern}"))
}

impl AutoSwitchRouter {
    fn new(
        rules: Vec<AutoSwitchRule>,
        upstream: UpstreamConfig,
        rule_list_cache: Option<&GfwListCache>,
    ) -> Result<Self, LxAppError> {
        let rule_list_router = match rule_list_cache {
            Some(cache) => Some(
                RuleListRouter::from_encoded(&cache.encoded, upstream.clone())
                    .map_err(|error| LxAppError::Runtime(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            rules,
            upstream,
            rule_list_router,
        })
    }
}

impl ProxyRouter for AutoSwitchRouter {
    fn route(&self, host: &str, port: u16) -> Result<RouteDecision, lingxia_proxy::ProxyError> {
        for rule in &self.rules {
            if host_matches_rule(host, &rule.pattern) {
                return Ok(RouteDecision::Upstream(match rule.action {
                    ProxyRuleAction::Proxy => self.upstream.clone(),
                    ProxyRuleAction::Direct => UpstreamConfig::Direct,
                }));
            }
        }

        if let Some(rule_list_router) = &self.rule_list_router {
            return rule_list_router.route(host, port);
        }

        Ok(RouteDecision::Upstream(UpstreamConfig::Direct))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn proxy_runtime_state() -> &'static Mutex<ProxyRuntimeState> {
    static STATE: OnceLock<Mutex<ProxyRuntimeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(ProxyRuntimeState::default()))
}

fn proxy_state_sender() -> &'static broadcast::Sender<ProxySettingsResult> {
    static TX: OnceLock<broadcast::Sender<ProxySettingsResult>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(32);
        tx
    })
}

fn lock_state() -> std::sync::MutexGuard<'static, ProxyRuntimeState> {
    proxy_runtime_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn local_proxy_addr_from_state() -> Option<String> {
    lock_state()
        .running
        .as_ref()
        .map(|running| running.local_addr.to_string())
}

fn update_runtime_snapshot(snapshot: ProxyRuntimeSnapshot) {
    lock_state().snapshot = Some(snapshot);
}

fn publish_proxy_state(state: &ProxySettingsResult) {
    let _ = proxy_state_sender().send(state.clone());
}

fn runtime_snapshot_for(settings: &ProxySettings, gfwlist: &GfwListMeta) -> ProxyRuntimeSnapshot {
    if let Some(snapshot) = lock_state().snapshot.clone() {
        return snapshot;
    }

    match settings.mode {
        ProxyMode::Direct => ProxyRuntimeSnapshot {
            is_active: false,
            status: "disabled".to_string(),
            status_message: "Direct connection is active.".to_string(),
            local_proxy_addr: None,
        },
        ProxyMode::Global => ProxyRuntimeSnapshot {
            is_active: false,
            status: "pending".to_string(),
            status_message: "Global proxy is configured but not applied yet.".to_string(),
            local_proxy_addr: None,
        },
        ProxyMode::GfwList => ProxyRuntimeSnapshot {
            is_active: false,
            status: "pending".to_string(),
            status_message: if gfwlist.ready {
                "Auto Switch is configured. Apply the profile to start rule-based routing."
                    .to_string()
            } else {
                "Auto Switch rules are not downloaded yet. Download rules first.".to_string()
            },
            local_proxy_addr: None,
        },
    }
}

fn map_webview_error(error: WebViewError) -> LxAppError {
    match error {
        WebViewError::InvalidCreateOptions(message) => LxAppError::InvalidParameter(message),
        WebViewError::WebView(message) => LxAppError::Runtime(message),
    }
}

fn activation_label(activation: ProxyActivation) -> &'static str {
    match activation {
        ProxyActivation::EffectiveNow => "Effective now",
        ProxyActivation::NewWebViewsOnly => "Applies to new WebViews only",
        ProxyActivation::EngineRecreateRequired => "Engine recreate required",
        ProxyActivation::NotApplied => "Not applied",
    }
}

fn snapshot_from_apply_report(
    report: ProxyApplyReport,
    local_proxy_addr: Option<SocketAddr>,
    mode_label: &str,
) -> ProxyRuntimeSnapshot {
    match report.status {
        ProxyApplyStatus::Applied => {
            let local = local_proxy_addr.map(|addr| addr.to_string());
            let mut message = match local.as_deref() {
                Some(addr) => format!(
                    "{} through local endpoint {}. {}.",
                    mode_label,
                    addr,
                    activation_label(report.activation)
                ),
                None => format!("{}. {}", mode_label, activation_label(report.activation)),
            };
            if let Some(detail) = report.detail
                && !detail.is_empty()
            {
                message.push(' ');
                message.push_str(&detail);
            }
            ProxyRuntimeSnapshot {
                is_active: true,
                status: "active".to_string(),
                status_message: message,
                local_proxy_addr: local,
            }
        }
        ProxyApplyStatus::Unsupported => ProxyRuntimeSnapshot {
            is_active: false,
            status: "unsupported".to_string(),
            status_message: report
                .detail
                .unwrap_or_else(|| "Current platform does not support WebView proxy.".to_string()),
            local_proxy_addr: local_proxy_addr.map(|addr| addr.to_string()),
        },
        ProxyApplyStatus::Cleared => ProxyRuntimeSnapshot {
            is_active: false,
            status: "disabled".to_string(),
            status_message: "Direct connection is active.".to_string(),
            local_proxy_addr: None,
        },
    }
}

fn snapshot_from_error(error: impl Into<String>) -> ProxyRuntimeSnapshot {
    ProxyRuntimeSnapshot {
        is_active: false,
        status: "error".to_string(),
        status_message: error.into(),
        local_proxy_addr: local_proxy_addr_from_state(),
    }
}

fn normalized_proxy_settings(input: ProxySettingsInput) -> ProxySettings {
    ProxySettings {
        mode: input.mode,
        enabled: !matches!(input.mode, ProxyMode::Direct),
        socks_host: input.socks5_host.trim().to_string(),
        socks_port: if input.socks5_port == 0 {
            DEFAULT_PROXY_SOCKS_PORT
        } else {
            input.socks5_port
        },
        username: input.username.trim().to_string(),
        password: input.password,
        gfwlist_source_url: input.gfwlist_source_url,
        auto_switch_rules: input.auto_switch_rules,
    }
    .normalized()
}

fn validate_proxy_settings(settings: &ProxySettings) -> Result<(), LxAppError> {
    if matches!(settings.mode, ProxyMode::Direct) {
        return Ok(());
    }

    validate_upstream_settings(settings).and_then(|_| validate_auto_switch_rules(settings))
}

fn validate_upstream_settings(settings: &ProxySettings) -> Result<(), LxAppError> {
    if settings.socks_host.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "SOCKS host is required for the selected proxy mode".to_string(),
        ));
    }
    if settings.socks_host.contains(char::is_whitespace) {
        return Err(LxAppError::InvalidParameter(
            "SOCKS host must not contain whitespace".to_string(),
        ));
    }
    if settings.socks_port == 0 {
        return Err(LxAppError::InvalidParameter(
            "SOCKS port must be greater than 0".to_string(),
        ));
    }

    Ok(())
}

fn validate_gfwlist_source(settings: &ProxySettings) -> Result<(), LxAppError> {
    validate_source_url(&settings.gfwlist_source_url)
        .map_err(|error| LxAppError::InvalidParameter(error.to_string()))
}

fn validate_auto_switch_rules(settings: &ProxySettings) -> Result<(), LxAppError> {
    for rule in &settings.auto_switch_rules {
        if rule.pattern.trim().is_empty() {
            return Err(LxAppError::InvalidParameter(
                "Auto Switch rules must have a domain pattern".to_string(),
            ));
        }
        if rule.pattern.contains(char::is_whitespace) {
            return Err(LxAppError::InvalidParameter(
                "Auto Switch rule patterns must not contain whitespace".to_string(),
            ));
        }
    }
    Ok(())
}

fn build_upstream_config(settings: &ProxySettings) -> UpstreamConfig {
    let credentials = (!settings.username.is_empty() || !settings.password.is_empty()).then(|| {
        Socks5Credentials {
            username: settings.username.clone(),
            password: settings.password.clone(),
        }
    });

    UpstreamConfig::Socks5 {
        host: settings.socks_host.clone(),
        port: settings.socks_port,
        credentials,
    }
}

fn gfwlist_cache_path(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, "proxy-gfwlist.json")
}

fn load_gfwlist_cache(app_data_dir: &Path) -> Result<Option<GfwListCache>, LxAppError> {
    let path = gfwlist_cache_path(app_data_dir);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(LxAppError::Runtime(format!(
                "failed to read GFW List cache: {}",
                error
            )));
        }
    };

    serde_json::from_slice::<GfwListCache>(&bytes)
        .map(Some)
        .map_err(|error| LxAppError::Runtime(format!("failed to parse GFW List cache: {}", error)))
}

fn save_gfwlist_cache(app_data_dir: &Path, cache: &GfwListCache) -> Result<(), LxAppError> {
    let path = gfwlist_cache_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            LxAppError::Runtime(format!(
                "failed to create GFW List cache directory: {}",
                error
            ))
        })?;
    }
    let bytes = serde_json::to_vec_pretty(cache).map_err(|error| {
        LxAppError::Runtime(format!("failed to encode GFW List cache: {}", error))
    })?;
    std::fs::write(&path, bytes)
        .map_err(|error| LxAppError::Runtime(format!("failed to write GFW List cache: {}", error)))
}

fn gfwlist_meta_from_cache_result(
    cache_result: Result<Option<GfwListCache>, LxAppError>,
) -> (Option<GfwListCache>, GfwListMeta) {
    match cache_result {
        Ok(Some(cache)) => {
            let meta = GfwListMeta {
                ready: true,
                updated_at_ms: Some(cache.updated_at_ms),
                status: "ready".to_string(),
                status_message: "Rules are cached locally and ready to use.".to_string(),
            };
            (Some(cache), meta)
        }
        Ok(None) => (
            None,
            GfwListMeta {
                ready: false,
                updated_at_ms: None,
                status: "empty".to_string(),
                status_message: "Rules have not been downloaded yet.".to_string(),
            },
        ),
        Err(error) => (
            None,
            GfwListMeta {
                ready: false,
                updated_at_ms: None,
                status: "error".to_string(),
                status_message: error.to_string(),
            },
        ),
    }
}

fn start_local_proxy() -> Result<RunningProxy, LxAppError> {
    // Serialize startup: without this guard two concurrent callers can both
    // observe `running == None`, spawn two proxy threads, and the loser leaks
    // a bound listener forever. This function already blocks (recv_timeout),
    // so holding a std mutex across check+spawn+recv+commit is fine.
    static START_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    let _start_guard = START_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    {
        let state = lock_state();
        if let Some(running) = state.running.clone() {
            log::info!(
                "[BrowserShellProxy] reusing local proxy listener at {}",
                running.local_addr
            );
            return Ok(running);
        }
    }

    log::info!("[BrowserShellProxy] starting local proxy listener on 127.0.0.1:0");
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("lingxia-local-proxy".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = tx.send(Err(format!("failed to build proxy runtime: {}", error)));
                    return;
                }
            };

            let proxy = match runtime.block_on(async {
                let router = Arc::new(FixedRouter(UpstreamConfig::Direct));
                LocalProxy::bind("127.0.0.1:0", router).await
            }) {
                Ok(proxy) => proxy,
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    return;
                }
            };

            let running = RunningProxy {
                local_addr: proxy.local_addr(),
                proxy: proxy.clone(),
            };
            log::info!(
                "[BrowserShellProxy] local proxy listener started at {}",
                running.local_addr
            );
            let _ = tx.send(Ok(running));
            runtime.block_on(async move {
                proxy.run().await;
            });
        })
        .map_err(|error| LxAppError::Runtime(format!("failed to start proxy thread: {}", error)))?;

    let running = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| LxAppError::Runtime(format!("failed to start local proxy: {}", error)))?
        .map_err(LxAppError::Runtime)?;

    // START_GUARD is held, so no concurrent starter can have raced us here.
    lock_state().running = Some(running.clone());
    Ok(running)
}

fn apply_local_proxy_router(
    router: Arc<dyn ProxyRouter>,
    mode_label: &str,
) -> Result<ProxyRuntimeSnapshot, LxAppError> {
    let running = start_local_proxy()?;
    log::info!(
        "[BrowserShellProxy] applying {} via local endpoint {}",
        mode_label,
        running.local_addr
    );
    running.proxy.set_router(router);

    let local_proxy = ProxyConfig::new(
        running.local_addr.ip().to_string(),
        running.local_addr.port(),
    )
    .map_err(map_webview_error)?;
    webview_runtime::configure_proxy_for_new_webviews(Some(local_proxy))
        .map_err(map_webview_error)?;
    log::info!("[BrowserShellProxy] configured desired webview proxy for new WebViews");
    Ok(snapshot_from_apply_report(
        ProxyApplyReport::applied(ProxyActivation::NewWebViewsOnly),
        Some(running.local_addr),
        mode_label,
    ))
}

fn clear_webview_proxy() -> Result<ProxyRuntimeSnapshot, LxAppError> {
    log::info!("[BrowserShellProxy] clearing desired webview proxy");
    webview_runtime::configure_proxy_for_new_webviews(None).map_err(map_webview_error)?;
    log::info!("[BrowserShellProxy] cleared desired webview proxy for new WebViews");
    Ok(snapshot_from_apply_report(
        ProxyApplyReport::cleared(ProxyActivation::NewWebViewsOnly),
        None,
        "Direct connection",
    ))
}

fn apply_proxy_settings(
    settings: &ProxySettings,
    gfwlist_cache: Option<&GfwListCache>,
) -> Result<ProxyRuntimeSnapshot, LxAppError> {
    log::info!(
        "[BrowserShellProxy] apply_proxy_settings mode={} host={} port={} gfwlist_source={}",
        mode_label(settings.mode),
        settings.socks_host,
        settings.socks_port,
        settings.gfwlist_source_url
    );
    validate_proxy_settings(settings)?;

    match settings.mode {
        ProxyMode::Direct => clear_webview_proxy(),
        ProxyMode::Global => {
            let router: Arc<dyn ProxyRouter> =
                Arc::new(FixedRouter(build_upstream_config(settings)));
            apply_local_proxy_router(router, "Global proxy is active")
        }
        ProxyMode::GfwList => {
            if gfwlist_cache.is_none() && settings.auto_switch_rules.is_empty() {
                log::warn!(
                    "[BrowserShellProxy] Auto Switch selected but neither cached rule list nor custom rules are available"
                );
                let _ = clear_webview_proxy();
                return Ok(ProxyRuntimeSnapshot {
                    is_active: false,
                    status: "pending".to_string(),
                    status_message:
                        "Auto Switch has no rule list or custom rules yet. Download rules or add a custom rule first."
                            .to_string(),
                    local_proxy_addr: None,
                });
            }
            if let Some(cache) = gfwlist_cache {
                log::info!(
                    "[BrowserShellProxy] applying GFW List router with cached rules updated_at_ms={}",
                    cache.updated_at_ms
                );
            } else {
                log::warn!(
                    "[BrowserShellProxy] GFW List mode selected but no cached rule list is available"
                );
            }
            let router: Arc<dyn ProxyRouter> = Arc::new(AutoSwitchRouter::new(
                settings.auto_switch_rules.clone(),
                build_upstream_config(settings),
                gfwlist_cache,
            )?);
            apply_local_proxy_router(router, "Auto Switch is active")
        }
    }
}

fn settings_result(
    settings: ProxySettings,
    snapshot: ProxyRuntimeSnapshot,
    gfwlist: GfwListMeta,
) -> ProxySettingsResult {
    ProxySettingsResult {
        mode: settings.mode,
        socks5_host: settings.socks_host,
        socks5_port: settings.socks_port,
        username: settings.username,
        password: settings.password,
        auto_switch_rules: settings.auto_switch_rules,
        is_active: snapshot.is_active,
        status: snapshot.status,
        status_message: snapshot.status_message,
        local_proxy_addr: snapshot.local_proxy_addr,
        gfwlist_ready: gfwlist.ready,
        gfwlist_source_url: settings.gfwlist_source_url.clone(),
        gfwlist_updated_at_ms: gfwlist.updated_at_ms,
        gfwlist_status: gfwlist.status,
        gfwlist_status_message: gfwlist.status_message,
    }
}

fn saved_snapshot_for_save(
    settings: &ProxySettings,
    gfwlist: &GfwListMeta,
) -> ProxyRuntimeSnapshot {
    match settings.mode {
        ProxyMode::Direct => ProxyRuntimeSnapshot {
            is_active: false,
            status: "saved".to_string(),
            status_message: "Saved. Direct mode is configured.".to_string(),
            local_proxy_addr: local_proxy_addr_from_state(),
        },
        ProxyMode::Global => ProxyRuntimeSnapshot {
            is_active: false,
            status: "saved".to_string(),
            status_message: "Saved. Always Proxy is configured.".to_string(),
            local_proxy_addr: local_proxy_addr_from_state(),
        },
        ProxyMode::GfwList => {
            if gfwlist.ready || !settings.auto_switch_rules.is_empty() {
                ProxyRuntimeSnapshot {
                    is_active: false,
                    status: "saved".to_string(),
                    status_message: "Saved. Auto Switch is configured.".to_string(),
                    local_proxy_addr: local_proxy_addr_from_state(),
                }
            } else {
                ProxyRuntimeSnapshot {
                    is_active: false,
                    status: "saved".to_string(),
                    status_message:
                        "Saved. Download rules or add a custom rule to activate Auto Switch."
                            .to_string(),
                    local_proxy_addr: None,
                }
            }
        }
    }
}

fn load_proxy_settings(app_data_dir: &Path) -> Result<ProxySettings, LxAppError> {
    crate::proxy_settings::load_proxy_settings(app_data_dir)
        .map(|settings| settings.normalized())
        .map_err(|error| LxAppError::Runtime(error.to_string()))
}

fn get_proxy_settings_result(app_data_dir: &Path) -> Result<ProxySettingsResult, LxAppError> {
    let settings = load_proxy_settings(app_data_dir)?;
    let (_, gfwlist_meta) = gfwlist_meta_from_cache_result(load_gfwlist_cache(app_data_dir));
    let snapshot = runtime_snapshot_for(&settings, &gfwlist_meta);
    Ok(settings_result(settings, snapshot, gfwlist_meta))
}

/// Monotonic generation for settings updates: each successful save bumps it,
/// and the matching background apply is skipped when a newer save exists.
/// This removes the last-writer-wins nondeterminism between concurrent
/// `proxy.updateSettings` calls.
static PROXY_APPLY_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Serializes writes to the proxy settings file so the file's final content
/// always corresponds to the highest `PROXY_APPLY_GENERATION` value.
static PROXY_SETTINGS_SAVE_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn save_proxy_settings_and_schedule_apply(
    app_data_dir: PathBuf,
    input: ProxySettingsInput,
) -> Result<ProxySettingsResult, LxAppError> {
    let settings = normalized_proxy_settings(input);
    log::info!(
        "[BrowserShellProxy] updateSettings requested: mode={} host={} port={} gfwlist_source={}",
        mode_label(settings.mode),
        settings.socks_host,
        settings.socks_port,
        settings.gfwlist_source_url
    );
    let (_gfwlist_cache, gfwlist_meta) =
        gfwlist_meta_from_cache_result(load_gfwlist_cache(&app_data_dir));

    if let Err(error) = validate_proxy_settings(&settings) {
        log::warn!(
            "[BrowserShellProxy] updateSettings validation failed: {}",
            error
        );
        return Ok(settings_result(
            settings,
            snapshot_from_error(error.to_string()),
            gfwlist_meta,
        ));
    }

    // Hold the save mutex across save + generation bump so concurrent updates
    // assign generations in the same order as their file writes.
    let apply_generation = {
        let _save_guard = PROXY_SETTINGS_SAVE_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(error) = crate::proxy_settings::save_proxy_settings(&app_data_dir, &settings) {
            log::warn!(
                "[BrowserShellProxy] failed to persist proxy settings: {}",
                error
            );
            return Ok(settings_result(
                settings,
                snapshot_from_error(error.to_string()),
                gfwlist_meta,
            ));
        }
        PROXY_APPLY_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
    };

    let saved_snapshot = saved_snapshot_for_save(&settings, &gfwlist_meta);
    update_runtime_snapshot(saved_snapshot.clone());

    let settings_for_apply = settings.clone();
    let app_data_dir_for_apply = app_data_dir.clone();
    std::mem::drop(rong::RongExecutor::global().spawn_blocking(move || {
        if PROXY_APPLY_GENERATION.load(Ordering::SeqCst) != apply_generation {
            log::info!(
                "[BrowserShellProxy] skipping stale background apply (generation {})",
                apply_generation
            );
            return;
        }
        let (gfwlist_cache, _gfwlist_meta) =
            gfwlist_meta_from_cache_result(load_gfwlist_cache(&app_data_dir_for_apply));
        let snapshot = match apply_proxy_settings(&settings_for_apply, gfwlist_cache.as_ref()) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::warn!(
                    "[BrowserShellProxy] background apply_proxy_settings failed: {}",
                    error
                );
                snapshot_from_error(error.to_string())
            }
        };
        log::info!(
            "[BrowserShellProxy] background apply completed: mode={} status={} active={}",
            mode_label(settings_for_apply.mode),
            snapshot.status,
            snapshot.is_active
        );
        if PROXY_APPLY_GENERATION.load(Ordering::SeqCst) != apply_generation {
            log::info!(
                "[BrowserShellProxy] discarding stale background apply result (generation {})",
                apply_generation
            );
            return;
        }
        update_runtime_snapshot(snapshot);
        match get_proxy_settings_result(&app_data_dir_for_apply) {
            Ok(result) => publish_proxy_state(&result),
            Err(error) => log::warn!(
                "[BrowserShellProxy] failed to publish proxy state after background apply: {}",
                error
            ),
        }
    }));

    let result = settings_result(settings, saved_snapshot, gfwlist_meta);
    publish_proxy_state(&result);
    Ok(result)
}

async fn refresh_gfwlist_result(app_data_dir: &Path) -> Result<ProxySettingsResult, LxAppError> {
    let settings = load_proxy_settings(app_data_dir)?;
    log::info!(
        "[BrowserShellProxy] refreshGfwList requested: source={} host={} port={}",
        settings.gfwlist_source_url,
        settings.socks_host,
        settings.socks_port
    );
    validate_upstream_settings(&settings)?;
    validate_gfwlist_source(&settings)?;

    let upstream = build_upstream_config(&settings);
    let encoded = fetch_encoded_from_url(&settings.gfwlist_source_url, &upstream)
        .await
        .map_err(|error| {
            log::warn!(
                "[BrowserShellProxy] refreshGfwList download failed: {}",
                error
            );
            LxAppError::Runtime(error.to_string())
        })?;
    let cache = GfwListCache {
        encoded,
        updated_at_ms: now_ms(),
    };
    log::info!(
        "[BrowserShellProxy] refreshGfwList downloaded rules successfully updated_at_ms={}",
        cache.updated_at_ms
    );
    save_gfwlist_cache(app_data_dir, &cache)?;

    if matches!(settings.mode, ProxyMode::GfwList) {
        // apply_proxy_settings blocks (proxy startup waits on recv_timeout), so
        // run it on the blocking pool just like proxy.updateSettings does.
        let task = rong::RongExecutor::global()
            .spawn_blocking(move || apply_proxy_settings(&settings, Some(&cache)));
        let snapshot = match task.await {
            Ok(Ok(snapshot)) => snapshot,
            Ok(Err(error)) => {
                log::warn!(
                    "[BrowserShellProxy] failed to apply GFW List immediately after refresh: {}",
                    error
                );
                snapshot_from_error(error.to_string())
            }
            Err(error) => {
                log::warn!(
                    "[BrowserShellProxy] GFW List apply task failed after refresh: {}",
                    error
                );
                snapshot_from_error(error.to_string())
            }
        };
        update_runtime_snapshot(snapshot);
    }

    let result = get_proxy_settings_result(app_data_dir)?;
    publish_proxy_state(&result);
    Ok(result)
}

#[lingxia::native("proxy.getSettings")]
fn get_proxy_settings(app: Arc<LxApp>) -> HostResult<ProxySettingsResult> {
    get_proxy_settings_result(&app.app_data_dir())
}

#[lingxia::native("proxy.updateSettings")]
async fn update_proxy_settings(
    app: Arc<LxApp>,
    input: ProxySettingsInput,
) -> HostResult<ProxySettingsResult> {
    let app_data_dir = app.app_data_dir();
    let task = rong::RongExecutor::global()
        .spawn_blocking(move || save_proxy_settings_and_schedule_apply(app_data_dir, input));
    match task.await {
        Ok(result) => {
            match &result {
                Ok(output) => log::info!(
                    "[BrowserShellProxy] proxy.updateSettings completed: mode={} status={} active={} gfwlist_ready={}",
                    mode_label(output.mode),
                    output.status,
                    output.is_active,
                    output.gfwlist_ready
                ),
                Err(error) => {
                    log::warn!(
                        "[BrowserShellProxy] proxy.updateSettings returned error: {}",
                        error
                    )
                }
            }
            result
        }
        Err(error) => {
            log::warn!(
                "[BrowserShellProxy] proxy.updateSettings task failed: {}",
                error
            );
            Err(LxAppError::Runtime(format!(
                "proxy.updateSettings task failed: {}",
                error
            )))
        }
    }
}

#[lingxia::native("proxy.refreshGfwList")]
async fn refresh_gfwlist(app: Arc<LxApp>) -> HostResult<ProxySettingsResult> {
    let result = refresh_gfwlist_result(&app.app_data_dir()).await;
    match &result {
        Ok(output) => log::info!(
            "[BrowserShellProxy] proxy.refreshGfwList completed: mode={} status={} gfwlist_ready={} updated_at_ms={:?}",
            mode_label(output.mode),
            output.status,
            output.gfwlist_ready,
            output.gfwlist_updated_at_ms
        ),
        Err(error) => log::warn!(
            "[BrowserShellProxy] proxy.refreshGfwList returned error: {}",
            error
        ),
    }
    result
}

#[lingxia::native("proxy.watch", stream)]
async fn watch_proxy_settings(
    app: Arc<LxApp>,
    mut stream: StreamContext<ProxySettingsResult>,
) -> HostResult<()> {
    // Subscribe before building the initial snapshot so state changes that
    // happen in between are not lost.
    let mut rx = proxy_state_sender().subscribe();
    let initial = get_proxy_settings_result(&app.app_data_dir())?;
    stream.send(initial)?;

    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            recv = rx.recv() => {
                match recv {
                    Ok(event) => stream.send(event)?,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        return Err(LxAppError::Bridge(format!(
                            "proxy stream lagged by {skipped} events"
                        )));
                    }
                    Err(broadcast::error::RecvError::Closed) => return stream.end(()),
                }
            }
        }
    }
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(get_proxy_settings_host());
    lxapp::host::register_host_entry(update_proxy_settings_host());
    lxapp::host::register_host_entry(refresh_gfwlist_host());
    lxapp::host::register_host_entry(watch_proxy_settings_host());
}

pub(crate) fn warmup() {
    let Some(runtime) = lxapp::get_platform() else {
        return;
    };
    let app_data_dir = runtime.app_data_dir();
    let settings = match load_proxy_settings(&app_data_dir) {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!(
                "[BrowserShellProxy] failed to load proxy settings: {}",
                error
            );
            update_runtime_snapshot(snapshot_from_error(error.to_string()));
            return;
        }
    };
    log::info!(
        "[BrowserShellProxy] warmup with mode={} host={} port={} gfwlist_source={}",
        mode_label(settings.mode),
        settings.socks_host,
        settings.socks_port,
        settings.gfwlist_source_url
    );

    let (gfwlist_cache, gfwlist_meta) =
        gfwlist_meta_from_cache_result(load_gfwlist_cache(&app_data_dir));

    let snapshot = match apply_proxy_settings(&settings, gfwlist_cache.as_ref()) {
        Ok(snapshot) => snapshot,
        Err(error) => snapshot_from_error(error.to_string()),
    };

    if matches!(settings.mode, ProxyMode::GfwList) && !gfwlist_meta.ready && !snapshot.is_active {
        let snapshot = ProxyRuntimeSnapshot {
            is_active: false,
            status: "pending".to_string(),
            status_message: "Auto Switch is selected. Download rules before applying it."
                .to_string(),
            local_proxy_addr: None,
        };
        update_runtime_snapshot(snapshot);
        if let Ok(result) = get_proxy_settings_result(&app_data_dir) {
            publish_proxy_state(&result);
        }
        return;
    }

    update_runtime_snapshot(snapshot);
    if let Ok(result) = get_proxy_settings_result(&app_data_dir) {
        publish_proxy_state(&result);
    }
}
