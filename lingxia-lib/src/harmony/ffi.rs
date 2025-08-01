use crate::harmony::app::App;
use crate::runtime::PlatformAppRuntime;
use lingxia_webview::schemehandler::register_custom_schemes;
use log::LevelFilter;
use lxapp::LxAppDelegate;
use lxapp::log::LogLevel;
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Object;
use napi_ohos::bindgen_prelude::*;
use ohos_hilog::Config;

/// NAPI-compatible LxApp information
#[napi(object)]
pub struct LxAppInfo {
    pub initial_route: String,
    pub app_name: String,
    pub debug: bool,
}

/// NAPI-compatible TabBar configuration
#[napi(object)]
pub struct TabBarConfig {
    pub color: String,
    pub selected_color: String,
    pub background_color: String,
    pub border_style: String,
    pub list: Vec<TabItem>,
    pub position: TabBarPosition,
    pub dimension: i32,
}

/// NAPI-compatible TabBar position enum
#[napi]
pub enum TabBarPosition {
    Bottom = 0,
    Top = 1,
    Left = 2,
    Right = 3,
}

/// NAPI-compatible TabItem
#[napi(object)]
pub struct TabItem {
    pub page_path: String,
    pub text: Option<String>,
    pub icon_path: Option<String>,
    pub selected_icon_path: Option<String>,
    pub selected: bool,
    pub group: i32, // 0=middle/center (default), 1=start (top/left), 2=end (bottom/right)
}

/// NAPI-compatible Navigation style enum
#[napi]
pub enum NavigationStyle {
    Default = 0,
    Custom = 1,
}

/// NAPI-compatible NavigationBar configuration
#[napi(object)]
pub struct NavigationBarConfig {
    pub navigation_bar_background_color: String,
    pub navigation_bar_text_style: String,
    pub navigation_bar_title_text: String,
    pub navigation_style: NavigationStyle,
}

#[napi]
pub fn lxapp_init(
    env: Env,
    callback_function: Function<'static>,
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
    lxapp::log::LogManager::init(|log_msg| {
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
        "Initializing LxApp with data_dir: {}, cache_dir: {}",
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
    if let Err(e) = lingxia_webview::tsfn::init(callback_function) {
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

    // Initialize global runtime and pass to lxapp::init
    let runtime = PlatformAppRuntime::init(app);

    // Return only the home app ID
    let home_app_id = lxapp::init(runtime)?;
    Some(home_app_id)
}

/// Get LxApp information
#[napi]
fn get_lx_app_info(appid: String) -> Option<LxAppInfo> {
    let lxapp = lxapp::get(appid);
    let app_config = lxapp.get_config();
    let rust_app_info = app_config.get_lxapp_info();

    Some(LxAppInfo {
        initial_route: rust_app_info.initial_route,
        app_name: rust_app_info.app_name,
        debug: rust_app_info.debug,
    })
}

/// Get tab bar configuration
#[napi]
fn get_tab_bar_config(appid: String) -> Option<TabBarConfig> {
    let lxapp = lxapp::get(appid);
    let app_config = lxapp.get_config();
    let rust_config = app_config.get_tab_bar_config(&lxapp)?;

    let position = match rust_config.position {
        lxapp::config::TabBarPosition::Bottom => TabBarPosition::Bottom,
        lxapp::config::TabBarPosition::Top => TabBarPosition::Top,
        lxapp::config::TabBarPosition::Left => TabBarPosition::Left,
        lxapp::config::TabBarPosition::Right => TabBarPosition::Right,
    };

    let list = rust_config
        .list
        .into_iter()
        .map(|item| TabItem {
            page_path: item.pagePath,
            text: item.text,
            icon_path: item.iconPath,
            selected_icon_path: item.selectedIconPath,
            selected: item.selected,
            group: match &item.group {
                Some(lxapp::config::TabItemGroup::Start) => 1,
                Some(lxapp::config::TabItemGroup::End) => 2,
                None => 0,
            },
        })
        .collect();

    Some(TabBarConfig {
        color: rust_config.color,
        selected_color: rust_config.selectedColor,
        background_color: rust_config.backgroundColor,
        border_style: rust_config.borderStyle,
        list,
        position,
        dimension: rust_config.dimension,
    })
}

/// Get page navigation bar configuration
#[napi]
pub fn get_navigation_bar_config(appid: String, path: String) -> NavigationBarConfig {
    let lxapp = lxapp::get(appid);
    let app_config = lxapp.get_config();
    let rust_config = app_config.get_nav_bar_config(&lxapp, &path);

    let navigation_style = match rust_config.navigationStyle {
        lxapp::config::NavigationStyle::Default => NavigationStyle::Default,
        lxapp::config::NavigationStyle::Custom => NavigationStyle::Custom,
    };

    NavigationBarConfig {
        navigation_bar_background_color: rust_config.navigationBarBackgroundColor,
        navigation_bar_text_style: rust_config.navigationBarTextStyle,
        navigation_bar_title_text: rust_config.navigationBarTitleText,
        navigation_style,
    }
}

/// Notify that LxApp was opened
#[napi]
pub fn on_lxapp_opened(appid: String, path: String) -> i32 {
    let lxapp = lxapp::get(appid);
    lxapp.on_lxapp_opened(path);
    0
}

/// Notify that LxApp was closed
#[napi]
pub fn on_lxapp_closed(appid: String) -> i32 {
    let lxapp = lxapp::get(appid);
    lxapp.on_lxapp_closed();
    0
}

/// Notify that a page is being shown
#[napi]
pub fn on_page_show(appid: String, path: String) -> i32 {
    let lxapp = lxapp::get(appid);
    lxapp.on_page_show(path);
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
    let lxapp = lxapp::get(appid);
    lxapp.on_page_scroll_changed(path, scroll_x, scroll_y, max_scroll_x, max_scroll_y);
    0
}
