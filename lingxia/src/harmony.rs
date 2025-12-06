use lingxia_messaging::invoke_callback;
use lingxia_platform::harmony::camera;
use lingxia_webview::{
    WebTag, get_webview_delegate, tsfn, webview_controller_created, webview_controller_destroyed,
};
use log::LevelFilter;
use lxapp::log::LogLevel;
use lxapp::{LxAppDelegate, UiEventType as LxAppUiEventType};
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Object;
use napi_ohos::bindgen_prelude::*;
use ohos_hilog::Config;

/// Parses a color string (e.g., "#RRGGBB" or "transparent") into a u32 ARGB value for Harmony.
fn parse_color_to_u32(color_str: &str, default_color: u32) -> u32 {
    if color_str.eq_ignore_ascii_case("transparent") {
        return 0x00000000;
    }

    if color_str.starts_with('#')
        && color_str.len() == 7
        && let Ok(rgb) = u32::from_str_radix(&color_str[1..], 16)
    {
        return 0xFF000000 | rgb; // Add full alpha
    }

    default_color
}

/// NAPI-compatible LxApp information
#[napi(object)]
pub struct LxAppInfo {
    pub app_name: String,
    pub cache_dir: String,
}

/// NAPI-compatible TabBar state with items array
#[napi(object)]
pub struct TabBarState {
    pub color: u32,
    pub selected_color: u32,
    pub background_color: u32,
    pub border_style: u32,
    pub position: TabBarPosition,
    pub dimension: i32,
    pub is_visible: bool,
    pub items: Vec<TabItem>,
    pub selected_index: i32,
}

/// NAPI-compatible TabBar position enum
#[napi]
pub enum TabBarPosition {
    Bottom = 0,
    Left = 1,
    Right = 2,
}

/// NAPI-compatible UI event type enum
#[napi]
pub enum UiEventType {
    TabBarClick = 0,
    CapsuleClick = 1,
    NavigationClick = 2,
    BackPress = 3,
}

/// NAPI-compatible TabItem
#[napi(object)]
pub struct TabItem {
    pub page_path: String,
    pub text: Option<String>,
    pub icon_path: Option<String>,
    pub selected_icon_path: Option<String>,
    pub selected: bool,
    pub group: i32, // 0=middle/center (default), 1=start (left), 2=end (right)
    pub badge: Option<String>, // Optional - only populated by get_tab_bar_item
    pub has_red_dot: Option<bool>, // Optional - only populated by get_tab_bar_item
}

/// NAPI-compatible NavigationBar state
#[napi(object)]
pub struct NavigationBarState {
    pub navigation_bar_background_color: u32,
    pub navigation_bar_text_style: String,
    pub navigation_bar_title_text: String,
    pub show_navbar: bool,
    pub show_back_button: bool,
    pub show_home_button: bool,
}

/// NAPI-compatible current LxApp information
#[napi(object)]
pub struct CurrentLxApp {
    pub appid: String,
    pub path: String,
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
    locale: String,
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
        "Initializing LxApp with data_dir: {}, cache_dir: {}, locale: {}",
        data_dir,
        cache_dir,
        locale
    );

    // Initialize TSFN
    if let Err(e) = tsfn::init(callback_function) {
        log::error!("Failed to initialize TSFN: {}", e);
        return None;
    }

    // Only create App if we have ResourceManager
    if resource_manager.is_none() {
        log::error!("ResourceManager is required but not provided");
        return None;
    }

    // Create Platform instance
    let platform = match lingxia_platform::Platform::new(
        data_dir.to_string(),
        cache_dir.to_string(),
        env,
        resource_manager,
        locale,
    ) {
        Ok(platform) => platform,
        Err(e) => {
            log::error!("Failed to create Platform: {}", e);
            return None;
        }
    };

    crate::init_with_platform(platform)
}

/// Register custom schemes (must be called before WebEngine initialization)
#[napi]
pub fn register_custom_schemes() -> bool {
    if let Err(e) = lingxia_webview::register_custom_schemes() {
        log::error!("Failed to register custom schemes: {}", e);
        false
    } else {
        true
    }
}

/// Get LxApp information
#[napi]
fn get_lx_app_info(appid: String) -> Option<LxAppInfo> {
    lxapp::try_get(&appid).map(|lxapp| {
        let rust_app_info = lxapp.get_lxapp_info();
        LxAppInfo {
            app_name: rust_app_info.app_name,
            cache_dir: lxapp.user_cache_dir.to_string_lossy().into_owned(),
        }
    })
}

/// Get complete TabBar state with items array
#[napi]
fn get_tab_bar(appid: String) -> Option<TabBarState> {
    lxapp::try_get(&appid).and_then(|lxapp| {
        lxapp.get_tabbar().map(|tabbar| {
            let items: Vec<TabItem> = tabbar
                .list
                .iter()
                .map(|item| TabItem {
                    page_path: item.pagePath.clone(),
                    text: item.text.clone(),
                    icon_path: item.iconPath.clone(),
                    selected_icon_path: item.selectedIconPath.clone(),
                    selected: item.selected,
                    group: match &item.group {
                        Some(lxapp::tabbar::TabItemGroup::Start) => 1,
                        Some(lxapp::tabbar::TabItemGroup::End) => 2,
                        None => 0,
                    },
                    badge: item.badge.clone(),
                    has_red_dot: Some(item.has_red_dot),
                })
                .collect();

            TabBarState {
                color: parse_color_to_u32(&tabbar.color, 0xFF666666),
                selected_color: parse_color_to_u32(&tabbar.selectedColor, 0xFF1677FF),
                background_color: parse_color_to_u32(&tabbar.backgroundColor, 0xFFFFFFFF),
                border_style: parse_color_to_u32(&tabbar.borderStyle, 0xFFF0F0F0),
                position: match tabbar.position {
                    lxapp::tabbar::TabBarPosition::Bottom => TabBarPosition::Bottom,
                    lxapp::tabbar::TabBarPosition::Left => TabBarPosition::Left,
                    lxapp::tabbar::TabBarPosition::Right => TabBarPosition::Right,
                },
                dimension: tabbar.dimension,
                is_visible: tabbar.is_visible,
                items,
                selected_index: tabbar.selected_index,
            }
        })
    })
}

/// Get page navigation bar state with boolean controls
#[napi]
pub fn get_navigation_bar_state(appid: String, path: String) -> Option<NavigationBarState> {
    lxapp::try_get(&appid).map(|lxapp| {
        let rust_state = lxapp.get_navbar_state(&path);

        NavigationBarState {
            navigation_bar_background_color: parse_color_to_u32(
                &rust_state.navigationBarBackgroundColor,
                0xFFFFFFFF,
            ),
            navigation_bar_text_style: rust_state.navigationBarTextStyle,
            navigation_bar_title_text: rust_state.navigationBarTitleText,
            show_navbar: rust_state.show_navbar,
            show_back_button: rust_state.show_back_button,
            show_home_button: rust_state.show_home_button,
        }
    })
}

/// Notify that LxApp was opened
#[napi]
pub fn on_lxapp_opened(appid: String, path: String) -> String {
    lxapp::try_get(&appid)
        .map(|lxapp| lxapp.on_lxapp_opened(path))
        .unwrap_or_default()
}

/// Notify that LxApp was closed
#[napi]
pub fn on_lxapp_closed(appid: String) -> i32 {
    if let Some(lxapp) = lxapp::try_get(&appid) {
        lxapp.on_lxapp_closed();
    }
    0
}

/// Notify that a page is being shown
#[napi]
pub fn on_page_show(appid: String, path: String) -> i32 {
    if let Some(lxapp) = lxapp::try_get(&appid) {
        lxapp.on_page_show(path);
    }
    0
}

/// Handle UI events from ArkTS
#[napi]
pub fn on_ui_event(appid: String, event_type: UiEventType, data: String) -> bool {
    let ui_event_type = match event_type {
        UiEventType::TabBarClick => LxAppUiEventType::TabBarClick,
        UiEventType::CapsuleClick => LxAppUiEventType::CapsuleClick,
        UiEventType::NavigationClick => LxAppUiEventType::NavigationClick,
        UiEventType::BackPress => LxAppUiEventType::BackPress,
    };

    lxapp::try_get(&appid)
        .map(|lxapp| lxapp.on_ui_event(ui_event_type, data))
        .unwrap_or(false)
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
    let webtag = WebTag::new(&appid, &path, None);
    if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.on_page_scroll_changed(scroll_x, scroll_y, max_scroll_x, max_scroll_y);
        return 0;
    }
    -1
}

/// Handle AppLink URL by processing the path without host
#[napi]
pub fn on_applink_received(applink_url: String) -> i32 {
    log::info!("[Harmony] AppLink received: {}", applink_url);
    0
}

/// Push: device token from ArkTS
#[napi]
pub fn on_push_token_received(token: String) -> i32 {
    log::info!("[Harmony] Push token received: {}", token);
    0
}

/// Push: link/message from ArkTS (trigger: 0=Background,1=Tap,2=Launch)
#[napi]
pub fn on_pushlink_received(url: String, trigger: i32) -> i32 {
    let trigger_name = match trigger {
        0 => "Background",
        1 => "Tap",
        2 => "Launch",
        _ => "Unknown",
    };
    log::info!(
        "[Harmony] Push Link received: {} (trigger: {})",
        url,
        trigger_name
    );
    0
}

/// Get current active LxApp ID and path from Rust stack
#[napi]
fn get_current_lxapp() -> CurrentLxApp {
    let (current_appid, current_path) = lxapp::get_current_lxapp();
    CurrentLxApp {
        appid: current_appid,
        path: current_path,
    }
}

/// Callback from platform (called from ArkTS)
#[napi]
fn on_callback(id: String, success: bool, data: String) -> bool {
    let id = match id.parse::<u64>() {
        Ok(parsed_id) => parsed_id,
        Err(_) => {
            log::error!("[HarmonyOS] Failed to parse callback ID: {}", id);
            return false;
        }
    };
    invoke_callback(id, success, data)
}

#[napi]
pub fn camera_init(surface_id: String, facing: String) -> bool {
    log::info!(
        "[Harmony.Camera] camera_init called: surfaceId={}, facing={}",
        surface_id,
        facing
    );
    match camera::camera_init(&surface_id, &facing) {
        Ok(v) => {
            log::info!("[Harmony.Camera] camera_init Ok: {}", v);
            v
        }
        Err(e) => {
            log::error!("[Harmony.Camera] camera_init Err: {}", e);
            false
        }
    }
}

#[napi]
pub fn camera_release() {
    camera::camera_release();
}

#[napi]
pub fn camera_switch_facing(is_back: bool) -> bool {
    camera::camera_switch_facing(is_back).unwrap_or(false)
}

#[napi]
pub fn camera_start_video_with_surface(surface_id: String) -> bool {
    camera::camera_start_video_with_surface(&surface_id).unwrap_or(false)
}

#[napi]
pub fn camera_video_output_start() -> bool {
    camera::camera_video_output_start().unwrap_or(false)
}

#[napi]
pub fn camera_video_output_stop_and_release() -> bool {
    camera::camera_video_output_stop_and_release().unwrap_or(false)
}

#[napi]
pub fn camera_start_photo_with_surface(
    surface_id: String,
    callback_id: String,
    cache_dir: String,
) -> bool {
    log::info!(
        "[Harmony.Camera] camera_start_photo_with_surface: surface_id={}, callback_id={}, cache_dir={}",
        surface_id,
        callback_id,
        cache_dir
    );
    camera::camera_start_photo_with_surface(&surface_id, &callback_id, &cache_dir).unwrap_or(false)
}

#[napi]
pub fn camera_take_photo() -> bool {
    camera::camera_take_photo().is_ok()
}

#[napi]
pub fn on_webview_controller_created(webtag: String) -> bool {
    match webview_controller_created(&webtag) {
        Ok(_) => true,
        Err(e) => {
            log::error!(
                "[Harmony] Failed to process webview created callback for {}: {}",
                webtag,
                e
            );
            false
        }
    }
}

#[napi]
pub fn on_webview_controller_destroyed(webtag: String) -> bool {
    webview_controller_destroyed(&webtag);
    true
}

// ============================================================================
// Video Player NAPI exports
// ============================================================================

/// Create a native video player instance
/// Returns the player pointer as a BigInt for use with XComponent
#[napi]
pub fn video_player_create(component_id: String, callback_id: i64) -> i64 {
    match lingxia_platform::harmony::video_player::create_player(&component_id, callback_id as u64)
    {
        Ok(ptr) => ptr as i64,
        Err(e) => {
            log::error!("[Harmony.VideoPlayer] Failed to create player: {}", e);
            0
        }
    }
}

/// Set media source for the video player (URL or file path)
#[napi]
pub fn video_player_set_url(component_id: String, url: String) -> bool {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            match p.set_source(&url) {
                Ok(_) => return true,
                Err(e) => {
                    log::error!("[Harmony.VideoPlayer] set_source failed: {:?}", e);
                    return false;
                }
            }
        }
    }
    false
}

/// Set video surface from surface ID
#[napi]
pub fn video_player_set_surface(component_id: String, surface_id: String) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_set_surface: component_id={}, surface_id={}",
        component_id,
        surface_id
    );
    match lingxia_platform::harmony::video_player::set_video_surface_from_id(
        &component_id,
        &surface_id,
    ) {
        Ok(_) => {
            log::info!("[Harmony.VideoPlayer] video_player_set_surface: success");
            true
        }
        Err(e) => {
            log::error!(
                "[Harmony.VideoPlayer] video_player_set_surface failed: {:?}",
                e
            );
            false
        }
    }
}

/// Rebind surface and resume playback (used for fullscreen swaps)
#[napi]
pub fn video_player_rebind_surface(
    component_id: String,
    surface_id: String,
    position_ms: i32,
    should_play: bool,
) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_rebind_surface: component_id={}, surface_id={}, pos={}, play={}",
        component_id,
        surface_id,
        position_ms,
        should_play
    );
    match lingxia_platform::harmony::video_player::rebind_surface_from_id(
        &component_id,
        &surface_id,
        position_ms,
        should_play,
    ) {
        Ok(_) => true,
        Err(e) => {
            log::error!(
                "[Harmony.VideoPlayer] video_player_rebind_surface failed: {:?}",
                e
            );
            false
        }
    }
}

/// Prepare the video player
#[napi]
pub fn video_player_prepare(component_id: String) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_prepare: component_id={}",
        component_id
    );
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.prepare().is_ok();
        }
    }
    false
}

/// Start playback
#[napi]
pub fn video_player_play(component_id: String) -> bool {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.play().is_ok();
        }
    }
    false
}

/// Pause playback
#[napi]
pub fn video_player_pause(component_id: String) -> bool {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.pause().is_ok();
        }
    }
    false
}

/// Stop playback
#[napi]
pub fn video_player_stop(component_id: String) -> bool {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.stop().is_ok();
        }
    }
    false
}

/// Seek to position in milliseconds
#[napi]
pub fn video_player_seek(component_id: String, position_ms: i32) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_seek: component_id={}, position_ms={}",
        component_id,
        position_ms
    );
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            // Use PreviousSync for better compatibility - seeks to nearest keyframe before target
            // Closest mode might have issues on some devices/video formats
            return p
                .seek(
                    position_ms,
                    lingxia_platform::harmony::video_player::AVPlayerSeekMode::PreviousSync,
                )
                .is_ok();
        }
    }
    false
}

/// Set volume (0.0 to 1.0)
#[napi]
pub fn video_player_set_volume(component_id: String, volume: f64) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_set_volume: component_id={}, volume={}",
        component_id,
        volume
    );
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.set_volume(volume as f32).is_ok();
        }
    }
    false
}

/// Set looping
#[napi]
pub fn video_player_set_loop(component_id: String, looping: bool) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_set_loop: component_id={}, looping={}",
        component_id,
        looping
    );
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.set_looping(looping).is_ok();
        }
    }
    false
}

/// Get current playback position in milliseconds
#[napi]
pub fn video_player_get_current_time(component_id: String) -> i32 {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            return p.get_current_time().unwrap_or(0);
        }
    }
    0
}

/// Get duration in milliseconds
#[napi]
pub fn video_player_get_duration(component_id: String) -> i32 {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(p) = player.lock() {
            return p.get_duration().unwrap_or(0);
        }
    }
    0
}

/// Get video width in pixels
#[napi]
pub fn video_player_get_video_width(component_id: String) -> i32 {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(p) = player.lock() {
            return p.get_video_size().map(|(w, _)| w).unwrap_or(0);
        }
    }
    0
}

/// Get video height in pixels
#[napi]
pub fn video_player_get_video_height(component_id: String) -> i32 {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(p) = player.lock() {
            return p.get_video_size().map(|(_, h)| h).unwrap_or(0);
        }
    }
    0
}

/// Destroy the video player
#[napi]
pub fn video_player_destroy(component_id: String) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_destroy: component_id={}",
        component_id
    );
    lingxia_platform::harmony::video_player::destroy_player(&component_id).is_ok()
}
