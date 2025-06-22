use crate::controller::Controller;
use crate::harmony::app::App;
use log::LevelFilter;
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::*;
use ohos_hilog::Config;
use std::sync::mpsc;

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
            .with_max_level(LevelFilter::Trace) // limit log level
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
        "Initializing MiniApp with callback_function, data_dir: {}, cache_dir: {}",
        data_dir,
        cache_dir,
    );

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

    // Create a channel to receive the result from the closure
    let (tx, rx) = mpsc::channel::<Option<(String, String)>>();

    if !Controller::run(
        move |controller| -> bool {
            let result_option = miniapp::init(controller);

            // Send the result back to the main thread
            if tx.send(result_option).is_err() {
                log::error!("Failed to send init result: Receiver dropped?");
            }

            true
        },
        app,
    ) {
        log::error!("Controller::run reported failure (returned false).");
        let _ = rx.recv();
        return None;
    }

    let final_init_details = match rx.recv() {
        Ok(details_option) => details_option,
        Err(e) => {
            log::error!("Failed to receive result from channel: {}", e);
            None
        }
    };

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
