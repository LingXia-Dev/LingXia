use lingxia_messaging::invoke_callback;
use lxapp::LxAppInfo as CoreLxAppInfo;
use lxapp::log::LogLevel;
use lxapp::{LxAppDelegate, UiEventType};

/// Parses a color string (e.g., "#RRGGBB" or "transparent") into a u32 ARGB value.
fn parse_color_to_u32(color_str: &str, default_color: u32) -> u32 {
    if color_str.eq_ignore_ascii_case("transparent") {
        return 0x00000000;
    }

    if color_str.starts_with('#') {
        let hex_part = &color_str[1..];
        if hex_part.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex_part, 16) {
                return 0xFF000000 | rgb; // Add full alpha
            }
        }
    }
    default_color
}

#[swift_bridge::bridge]
mod bridge {
    // LxApp basic information for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct LxAppInfo {
        pub app_name: String,
    }

    // NavigationBar state for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct NavigationBarState {
        pub background_color: u32,
        pub text_style: String,
        pub title_text: String,
        pub show_navbar: bool,
        pub show_back_button: bool,
        pub show_home_button: bool,
    }

    // TabBar state for Swift (without items array)
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBar {
        pub color: u32,
        pub selected_color: u32,
        pub background_color: u32,
        pub border_style: u32,
        pub position: i32,
        pub dimension: i32,
        pub items_count: i32,
        pub is_visible: bool,
        pub selected_index: i32,
    }

    // Group alignment types
    pub enum GroupAlignment {
        Center, // 0=middle/center (default)
        Start,  // 1=start (top/left)
        End,    // 2=end (bottom/right)
    }

    // TabBar item for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBarItem {
        pub page_path: String,
        pub text: String,
        pub icon_path: String,
        pub selected_icon_path: String,
        pub selected: bool,
        pub group: GroupAlignment,
        pub badge: String,
        pub has_red_dot: bool,
    }

    // Push notification trigger types
    pub enum PushTrigger {
        Background,
        Tap,
        Launch,
    }

    // UI event types for unified event handling
    pub enum UiEventType {
        TabBarClick,
        CapsuleClick,
        NavigationClick,
        BackPress,
    }

    // Current LxApp info from Rust stack
    #[swift_bridge(swift_repr = "struct")]
    pub struct CurrentLxApp {
        pub appid: String,
        pub path: String,
    }

    extern "Rust" {
        #[swift_bridge(swift_name = "lxappInit")]
        fn lxapp_init(data_dir: &str, cache_dir: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onPageShow")]
        fn on_page_show(appid: &str, path: &str);

        #[swift_bridge(swift_name = "onLxappClosed")]
        fn on_lxapp_closed(appid: &str) -> i32;

        #[swift_bridge(swift_name = "getLxAppInfo")]
        fn get_lxapp_info(appid: &str) -> LxAppInfo;

        #[swift_bridge(swift_name = "getNavigationBarState")]
        fn get_navigation_bar_state(appid: &str, path: &str) -> NavigationBarState;

        #[swift_bridge(swift_name = "getTabBar")]
        fn get_tab_bar(appid: &str) -> Option<TabBar>;

        #[swift_bridge(swift_name = "getTabBarItem")]
        fn get_tab_bar_item(appid: &str, index: i32) -> Option<TabBarItem>;

        #[swift_bridge(swift_name = "onUiEvent")]
        fn on_ui_event(appid: &str, event_type: UiEventType, data: &str) -> bool;

        #[swift_bridge(swift_name = "onLxappOpened")]
        fn on_lxapp_opened(appid: &str, path: &str) -> String;

        #[swift_bridge(swift_name = "findWebView")]
        fn find_webview(appid: &str, path: &str) -> usize;

        #[swift_bridge(swift_name = "onApplinkReceived")]
        fn on_applink_received(applink_path: &str) -> i32;

        #[swift_bridge(swift_name = "getCurrentLxApp")]
        fn get_current_lxapp() -> CurrentLxApp;

        #[swift_bridge(swift_name = "onPushlinkReceived")]
        fn on_pushlink_received(url: &str, trigger: PushTrigger) -> i32;

        #[swift_bridge(swift_name = "onPushTokenReceived")]
        fn on_push_token_received(token: &str) -> i32;

        #[swift_bridge(swift_name = "onCallback")]
        fn on_callback(id: u64, success: bool, data: &str) -> bool;
    }
}

// Conversion from core LxAppInfo to FFI LxAppInfo
impl From<CoreLxAppInfo> for bridge::LxAppInfo {
    fn from(core_info: CoreLxAppInfo) -> Self {
        Self {
            app_name: core_info.app_name,
        }
    }
}

/// Initialize the LxApp system for iOS/macOS
pub fn lxapp_init(data_dir: &str, cache_dir: &str) -> Option<String> {
    oslog::OsLogger::new("LingXia.Rust")
        .level_filter(log::LevelFilter::Info)
        .init()
        .unwrap();

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
        cache_dir
    );

    let platform =
        match lingxia_platform::Platform::new(data_dir.to_string(), cache_dir.to_string()) {
            Ok(platform) => platform,
            Err(e) => {
                log::error!("Failed to create Platform: {}", e);
                return None;
            }
        };

    lingxia_logic::register_logic_runtime();
    lxapp::init(platform)
}

/// Notify that a page is being shown
pub fn on_page_show(appid: &str, path: &str) {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_page_show(path.to_string());
}

/// Notify that LxApp was closed
pub fn on_lxapp_closed(appid: &str) -> i32 {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_lxapp_closed();
    0
}

/// Handle UI events from Swift
pub fn on_ui_event(appid: &str, event_type: bridge::UiEventType, data: &str) -> bool {
    let ui_event_type = match event_type {
        bridge::UiEventType::TabBarClick => UiEventType::TabBarClick,
        bridge::UiEventType::CapsuleClick => UiEventType::CapsuleClick,
        bridge::UiEventType::NavigationClick => UiEventType::NavigationClick,
        bridge::UiEventType::BackPress => UiEventType::BackPress,
    };

    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_ui_event(ui_event_type, data.to_string())
}

/// Get current active LxApp ID and path from Rust stack
pub fn get_current_lxapp() -> bridge::CurrentLxApp {
    let (current_appid, current_path) = lxapp::get_current_lxapp();
    bridge::CurrentLxApp {
        appid: current_appid,
        path: current_path,
    }
}

/// Notify that LxApp was opened
pub fn on_lxapp_opened(appid: &str, path: &str) -> String {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_lxapp_opened(path.to_string())
}

/// Find a WebView for the specified app and path
/// This is called from Swift to get a WebView instance pointer managed by Rust
/// Returns the usize pointer to the WebView, or 0 if not found
pub fn find_webview(appid: &str, path: &str) -> usize {
    // Create WebTag and use lingxia-webview's find_webview function
    let webtag = lingxia_webview::WebTag::new(appid, path);
    if let Some(webview) = lingxia_webview::find_webview(&webtag) {
        // WebView exists, return its pointer
        webview.get_swift_webview_ptr()
    } else {
        log::error!("💥 WebView not found for appid: {}, path: {}", appid, path);
        0
    }
}

/// Get LxApp information
pub fn get_lxapp_info(appid: &str) -> bridge::LxAppInfo {
    let lxapp = lxapp::get(appid.to_string());
    let lxapp_info = lxapp.get_lxapp_info();

    // Convert from core LxAppInfo to FFI LxAppInfo
    lxapp_info.into()
}

/// Get NavigationBar state
pub fn get_navigation_bar_state(appid: &str, path: &str) -> bridge::NavigationBarState {
    let lxapp = lxapp::get(appid.to_string());
    let nav_state = lxapp.get_navbar_state(path);

    let bg_color = parse_color_to_u32(&nav_state.navigationBarBackgroundColor, 0xFFFFFFFF);

    bridge::NavigationBarState {
        background_color: bg_color,
        text_style: nav_state.navigationBarTextStyle,
        title_text: nav_state.navigationBarTitleText,
        show_navbar: nav_state.show_navbar,
        show_back_button: nav_state.show_back_button,
        show_home_button: nav_state.show_home_button,
    }
}

/// Get TabBar state
pub fn get_tab_bar(appid: &str) -> Option<bridge::TabBar> {
    let lxapp = lxapp::get(appid.to_string());

    lxapp.get_tabbar().map(|tabbar| bridge::TabBar {
        color: parse_color_to_u32(&tabbar.color, 0xFF666666),
        selected_color: parse_color_to_u32(&tabbar.selectedColor, 0xFF1677FF),
        background_color: parse_color_to_u32(&tabbar.backgroundColor, 0xFFFFFFFF),
        border_style: parse_color_to_u32(&tabbar.borderStyle, 0xFFF0F0F0),
        position: tabbar.position.to_i32(),
        dimension: tabbar.dimension,
        items_count: tabbar.list.len() as i32,
        is_visible: tabbar.is_visible,
        selected_index: tabbar.selected_index,
    })
}

/// Get TabBar item by index
pub fn get_tab_bar_item(appid: &str, index: i32) -> Option<bridge::TabBarItem> {
    let lxapp = lxapp::get(appid.to_string());

    lxapp.get_tabbar().and_then(|tabbar| {
        tabbar.get_item(index).map(|item| bridge::TabBarItem {
            page_path: item.pagePath.clone(),
            text: item.text.clone().unwrap_or_default(),
            icon_path: item.iconPath.clone().unwrap_or_default(),
            selected_icon_path: item.selectedIconPath.clone().unwrap_or_default(),
            selected: item.selected,
            group: match &item.group {
                Some(lxapp::tabbar::TabItemGroup::Start) => bridge::GroupAlignment::Start,
                Some(lxapp::tabbar::TabItemGroup::End) => bridge::GroupAlignment::End,
                None => bridge::GroupAlignment::Center,
            },
            badge: item.badge.clone().unwrap_or_default(),
            has_red_dot: item.has_red_dot,
        })
    })
}

/// Handle AppLink URL by processing the path (Universal Link)
pub fn on_applink_received(url: &str) -> i32 {
    log::info!("[Apple] Universal Link received: {}", url);
    0
}

/// Handle Push Notification Link with trigger context
pub fn on_pushlink_received(url: &str, trigger: bridge::PushTrigger) -> i32 {
    let trigger_name = match trigger {
        bridge::PushTrigger::Background => "Background",
        bridge::PushTrigger::Tap => "Tap",
        bridge::PushTrigger::Launch => "Launch",
    };

    log::info!(
        "[Apple] Push Link received: {} (trigger: {})",
        url,
        trigger_name
    );

    match trigger {
        bridge::PushTrigger::Background => {
            log::info!("[Apple] Background push link - silent processing");
        }
        bridge::PushTrigger::Tap | bridge::PushTrigger::Launch => {
            log::info!("[Apple] User-initiated push link - navigate to page");
        }
    }

    0
}

/// Handle push notification device token
pub fn on_push_token_received(token: &str) -> i32 {
    log::info!("[Apple] Push token received: {}", token);
    0
}

/// Callback from platform (called from Swift/Objective-C)
pub fn on_callback(id: u64, success: bool, data: &str) -> bool {
    invoke_callback(id, success, data.to_string())
}
