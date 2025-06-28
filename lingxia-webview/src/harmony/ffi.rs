use crate::harmony::app::App;
use crate::harmony::schemehandler::register_custom_schemes;
use crate::runtime::SimpleAppRuntime;
use log::LevelFilter;
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Object;
use napi_ohos::bindgen_prelude::*;
use ohos_hilog::Config;

#[napi]
pub fn miniapp_init(
    env: Env,
    callback_function: Function,
    data_dir: String,
    cache_dir: String,
    #[napi(ts_arg_type = "resourceManager.ResourceManager | null")] resource_manager: Option<
        Object,
    >,
) -> Option<String> {
    ohos_hilog::init_once(
        Config::default()
            .with_max_level(LevelFilter::Info) // limit log level
            .with_tag("LingXia.Rust"),
    );

    // Initialize the new logging system
    miniapp::log::LogManager::init(|log_msg| {
        let formatted_message = format!(
            "[{}{}{}] {}",
            log_msg.tag.as_str(),
            log_msg
                .appid
                .as_ref()
                .map(|id| format!(":{}", id))
                .unwrap_or_default(),
            log_msg
                .path
                .as_ref()
                .map(|p| format!(":{}", p))
                .unwrap_or_default(),
            log_msg.message
        );

        // Use log macros directly now that we have set up the global logger
        match log_msg.level {
            LogLevel::Verbose | LogLevel::Debug => {
                log::debug!("{}", formatted_message);
            }
            LogLevel::Info => {
                log::info!("{}", formatted_message);
            }
            LogLevel::Warn => {
                log::warn!("{}", formatted_message);
            }
            LogLevel::Error => {
                log::error!("{}", formatted_message);
            }
        }
    });

    log::info!(
        "Initializing MiniApp with data_dir: {}, cache_dir: {}",
        data_dir,
        cache_dir,
    );

    // Register custom schemes globally
    // https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/web-scheme-handler
    if let Err(e) = register_custom_schemes() {
        log::error!("Failed to register custom schemes: {}", e);
        return None;
    }

    // Initialize TSFN
    if let Err(e) = crate::harmony::tsfn::init(callback_function) {
        log::error!("Failed to initialize TSFN: {}", e);
        return None;
    }

    // Only create App if we have ResourceManager
    if resource_manager.is_none() {
        log::error!("ResourceManager is required but not provided");
        return None;
    }

    // Create App instance
    let app = match App::new(
        data_dir.to_string(),
        cache_dir.to_string(),
        env,
        resource_manager,
    ) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to create App: {}", e);
            return None;
        }
    };

    // Initialize global runtime and pass to miniapp::init
    let runtime = SimpleAppRuntime::init(app);
    let final_init_details = miniapp::init(runtime);

    // Format and return the result
    match final_init_details {
        Some((home_app_id, initial_route)) => {
            let combined_details = format!("{}:{}", home_app_id, initial_route);
            log::info!("MiniApp initialization successful: {}", combined_details);
            Some(combined_details)
        }
        None => {
            log::error!("Failed to obtain MiniApp home app details during initialization.");
            None
        }
    }
}

/// Get tab bar configuration
#[napi]
fn get_tab_bar_config(appid: String) -> Option<String> {
    let miniapp = miniapp::get(appid);
    match miniapp.get_tab_bar_config() {
        Ok(config) => Some(config),
        Err(_) => None,
    }
}

/// Get page configuration
#[napi]
pub fn get_page_config(appid: String, path: String) -> Option<String> {
    let miniapp = miniapp::get(appid);
    match miniapp.get_page_config(&path) {
        Ok(config) => Some(config),
        Err(_) => None,
    }
}

/// Notify that MiniApp was opened
#[napi]
pub fn on_miniapp_opened(appid: String, path: String) -> i32 {
    let miniapp = miniapp::get(appid);
    miniapp.on_miniapp_opened(path);
    0
}

/// Notify that MiniApp was closed
#[napi]
pub fn on_miniapp_closed(appid: String) -> i32 {
    let miniapp = miniapp::get(appid);
    miniapp.on_miniapp_closed();
    0
}

/// Notify that a page is being shown
#[napi]
pub fn on_page_show(appid: String, path: String) -> i32 {
    let miniapp = miniapp::get(appid);
    miniapp.on_page_show(path);
    0
}

#[napi]
pub fn on_scroll_changed(
    appid: String,
    path: String,
    scroll_x: i32,
    scroll_y: i32,
    max_scroll_x: i32,
    max_scroll_y: i32,
) -> i32 {
    let miniapp = miniapp::get(appid);
    miniapp.on_page_scroll_changed(path, scroll_x, scroll_y, max_scroll_x, max_scroll_y);
    0
}
