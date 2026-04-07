use lingxia_messaging::invoke_callback;
use lingxia_platform::harmony::camera;
use lingxia_platform::traits::video_player::VideoPlayerCommand;
use lingxia_webview::platform::harmony as webview_harmony;
use lxapp::{LxAppDelegate, LxAppUiEventType, OrientationConfig, PageOrientation};
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Object;
use napi_ohos::bindgen_prelude::*;

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
    pub version: String,
    pub release_type: String,
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
    PullDownRefresh = 4,
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
    pub session_id: i64,
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
    crate::logging::init();

    log::info!(
        "Initializing LxApp with data_dir: {}, cache_dir: {}, locale: {}",
        data_dir,
        cache_dir,
        locale
    );

    // Initialize TSFN
    if let Err(e) = webview_harmony::tsfn::init(callback_function) {
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
    if let Err(e) = webview_harmony::register_custom_schemes() {
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
            version: rust_app_info.version,
            release_type: rust_app_info.release_type,
            cache_dir: lxapp.user_cache_dir.to_string_lossy().into_owned(),
        }
    })
}

/// Resolve a lx:// URI or sandbox path to a native-consumable URL/path.
///
/// - Accepts `lx://usercache/...`, `lx://userdata/...`, relative paths like `images/1.png`,
///   and absolute paths.
/// - Returns `file://...` for local filesystem paths on Harmony, or `null` if not accessible.
/// - Passes through `http(s)://...` unchanged.
#[napi]
pub fn resolve_lx_uri(appid: String, input: String) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }

    let lxapp = lxapp::try_get(&appid)?;

    let resolved = if let Some(path) = trimmed.strip_prefix("file://") {
        lxapp.resolve_accessible_path(path).ok()?
    } else {
        lxapp.resolve_accessible_path(trimmed).ok()?
    };

    Some(format!("file://{}", resolved.to_string_lossy()))
}

#[napi]
pub fn handle_browser_address_input(request_json: String) -> Option<String> {
    crate::browser::resolve_input_json(&request_json)
}

#[napi]
pub fn handle_browser_navigation_policy(request_json: String) -> Option<String> {
    crate::browser::classify_navigation_json(&request_json)
}

#[napi]
pub fn open_browser_tab(appid: String, session_id: i64, url: String) -> Option<String> {
    if session_id <= 0 {
        return None;
    }
    crate::browser::open_for_app(&appid, session_id as u64, &url, None).ok()
}

#[napi]
pub fn browser_tab_close(tab_id: String) -> bool {
    crate::browser::close(&tab_id).is_ok()
}

#[napi]
pub fn get_builtin_browser_app_id() -> String {
    crate::browser::APP_ID.to_string()
}

#[napi]
pub fn browser_tab_path_for_id(tab_id: String) -> String {
    crate::browser::tab_path(&tab_id)
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
pub fn on_lxapp_opened(appid: String, path: String, session_id: i64) -> String {
    if session_id <= 0 {
        log::warn!(
            "on_lxapp_opened called without valid session_id for {}",
            appid
        );
        return String::new();
    }
    lxapp::try_get(&appid)
        .map(|lxapp| lxapp.on_lxapp_opened(path, session_id as u64))
        .unwrap_or_default()
}

/// Notify that LxApp was closed
#[napi]
pub fn on_lxapp_closed(appid: String, session_id: i64) -> bool {
    if let Some(lxapp) = lxapp::try_get(&appid) {
        if session_id <= 0 {
            log::warn!(
                "on_lxapp_closed called without valid session_id for {}",
                appid
            );
            return false;
        }
        if session_id as u64 != lxapp.session_id() {
            return false;
        }
        lxapp.on_lxapp_closed(session_id as u64);
        return true;
    }
    false
}

/// Notify device orientation changes from host platform.
#[napi]
pub fn on_device_orientation_changed(appid: String, session_id: i64, value: String) -> bool {
    let Some(lxapp) = lxapp::try_get(&appid) else {
        return false;
    };

    if session_id <= 0 || lxapp.session_id() != session_id as u64 {
        return false;
    }

    let normalized = match value.as_str() {
        "portrait" => "portrait",
        "landscape" => "landscape",
        _ => return false,
    };

    let payload = format!(r#"{{"value":"{}"}}"#, normalized);
    lxapp::publish_app_event(&appid, "DeviceOrientationChange", Some(payload))
}

/// Get page orientation for a specific page path.
/// Returns: 0=auto, 1=portrait, 2=landscape, 3=reverse-portrait, 4=reverse-landscape
#[napi]
pub fn get_page_orientation(appid: String, path: String) -> i32 {
    let Some(lxapp_instance) = lxapp::try_get(&appid) else {
        return 0;
    };

    let orientation = lxapp_instance.get_page_orientation(&path);
    orientation_to_value(orientation)
}

fn orientation_to_value(orientation: OrientationConfig) -> i32 {
    match (orientation.mode, orientation.rotation) {
        (PageOrientation::Auto, _) => 0,
        (PageOrientation::Portrait, 180) => 3,
        (PageOrientation::Portrait, _) => 1,
        (PageOrientation::Landscape, 180) => 4,
        (PageOrientation::Landscape, _) => 2,
    }
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
pub fn on_lxapp_event(appid: String, event_type: UiEventType, data: String) -> bool {
    let ui_event_type = match event_type {
        UiEventType::TabBarClick => LxAppUiEventType::TabBarClick,
        UiEventType::CapsuleClick => LxAppUiEventType::CapsuleClick,
        UiEventType::NavigationClick => LxAppUiEventType::NavigationClick,
        UiEventType::BackPress => LxAppUiEventType::BackPress,
        UiEventType::PullDownRefresh => LxAppUiEventType::PullDownRefresh,
    };

    lxapp::try_get(&appid)
        .map(|lxapp| lxapp.on_lxapp_event(ui_event_type, data))
        .unwrap_or(false)
}

#[napi]
pub fn on_native_component_event(
    appid: String,
    path: String,
    component_id: String,
    event_name: String,
    payload_json: String,
    bindings_json: String,
) -> bool {
    lxapp::on_native_component_event(
        &appid,
        &path,
        &component_id,
        &event_name,
        &payload_json,
        &bindings_json,
    )
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
    crate::push::bind_push_token_for_ffi(token)
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
    let (current_appid, current_path, current_session_id) = lxapp::get_current_lxapp();
    CurrentLxApp {
        appid: current_appid,
        path: current_path,
        session_id: current_session_id as i64,
    }
}

/// Get runtime session id for a specific lxapp.
#[napi]
fn get_lxapp_session_id(appid: String) -> i64 {
    lxapp::try_get(&appid)
        .map(|lxapp| lxapp.session_id() as i64)
        .unwrap_or(0)
}

/// Callback from platform (called from ArkTS)
///
/// # Parameters
/// - `id`: Callback ID as string for correlating with pending operation
/// - `success`: Whether the operation completed successfully
/// - `data`: When `success=true`, contains JSON payload; when `success=false`, contains error code string (see i18n/err_code)
#[napi]
fn on_callback(id: String, success: bool, data: String) -> bool {
    let id = match id.parse::<u64>() {
        Ok(parsed_id) => parsed_id,
        Err(_) => {
            log::error!("[HarmonyOS] Failed to parse callback ID: {}", id);
            return false;
        }
    };

    let result = if success {
        Ok(data)
    } else {
        // Parse data as u32 error code, default to 1000 (unknown error) if failed
        Err(data.parse::<u32>().unwrap_or(1000))
    };

    if invoke_callback(id, result) {
        true
    } else {
        log::warn!("[Harmony] Callback not found for id={}", id);
        false
    }
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
pub fn camera_set_flash_mode(flash_on: bool) -> bool {
    camera::camera_set_flash_mode(flash_on).unwrap_or(false)
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
    match webview_harmony::webview_controller_created(&webtag) {
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
    webview_harmony::webview_controller_destroyed(&webtag);
    true
}

#[napi]
pub fn on_navigation_policy(webtag: String, url: String) -> bool {
    webview_harmony::check_navigation_policy(&webtag, &url)
}

#[napi]
pub fn on_download_start(
    webtag: String,
    url: String,
    user_agent: String,
    content_disposition: String,
    mime_type: String,
    content_length: i64,
) -> bool {
    webview_harmony::on_download_start(
        &webtag,
        &url,
        &user_agent,
        &content_disposition,
        &mime_type,
        content_length,
    )
}

#[napi]
pub fn on_load_error(webtag: String, url: String, error_code: i32, description: String) -> bool {
    webview_harmony::on_load_error(&webtag, &url, error_code, &description);
    true
}

#[napi]
pub fn is_pull_down_refresh_enabled(appid: String, path: String) -> bool {
    lxapp::is_pull_down_refresh_enabled(&appid, &path)
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

/// Store video surface ID without creating or updating AVPlayer (streaming mode)
#[napi]
pub fn video_player_store_surface(component_id: String, surface_id: String) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_store_surface: component_id={}, surface_id={}",
        component_id,
        surface_id
    );
    lingxia_platform::harmony::video_player::store_surface_id_only(&component_id, &surface_id);
    true
}

/// Clear stored video surface ID (streaming or player teardown)
#[napi]
pub fn video_player_clear_surface(component_id: String) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_clear_surface: component_id={}",
        component_id
    );
    lingxia_platform::harmony::video_player::clear_surface_id(&component_id);
    true
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

#[napi]
pub fn video_player_rebind_stream_surface(component_id: String, surface_id: String) -> bool {
    log::info!(
        "[Harmony.VideoPlayer] video_player_rebind_stream_surface: component_id={}, surface_id={}",
        component_id,
        surface_id
    );
    lingxia_platform::harmony::video_player::rebind_stream_surface(&component_id, &surface_id)
        .is_ok()
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
    lingxia_platform::harmony::video_player::dispatch_command(
        &component_id,
        VideoPlayerCommand::Play,
    )
    .is_ok()
}

/// Pause playback
#[napi]
pub fn video_player_pause(component_id: String) -> bool {
    lingxia_platform::harmony::video_player::dispatch_command(
        &component_id,
        VideoPlayerCommand::Pause,
    )
    .is_ok()
}

/// Stop playback
#[napi]
pub fn video_player_stop(component_id: String) -> bool {
    lingxia_platform::harmony::video_player::dispatch_command(
        &component_id,
        VideoPlayerCommand::Stop,
    )
    .is_ok()
}

/// Seek to position in milliseconds
#[napi]
pub fn video_player_seek(component_id: String, position_ms: f64) -> bool {
    // Sanity check: Prevent massive values (e.g. i64::MAX or timestamps) that cause logic layer overflow.
    // Limit seek to ~100 years (valid playback range). 3e12 ms.
    const MAX_SEEK_MS: f64 = 3_000_000_000_000.0;

    // Prevent NaN, Infinite, negative, or massive values
    if !position_ms.is_finite() || position_ms < 0.0 || position_ms > MAX_SEEK_MS {
        log::error!(
            "[Harmony.VideoPlayer] video_player_seek: invalid/out-of-range position_ms={} for component_id={}",
            position_ms,
            component_id
        );
        return false;
    }

    log::info!(
        "[Harmony.VideoPlayer] video_player_seek: component_id={}, position_ms={}",
        component_id,
        position_ms
    );

    if lingxia_platform::harmony::video_player::has_stream_decoder(&component_id) {
        let position_s = position_ms / 1000.0;

        // Call lxapp layer to perform actual stream seek (via registered callback)
        let seek_result = lingxia_media::seek_stream_session(&component_id, position_s);
        if !seek_result {
            log::warn!(
                "[Harmony.VideoPlayer] video_player_seek: stream seek failed, no callback registered for component_id={}",
                component_id
            );
        }

        // Also dispatch to platform layer for UI sync (emits seeked event)
        let _ = lingxia_platform::harmony::video_player::dispatch_command(
            &component_id,
            VideoPlayerCommand::Seek {
                position: position_s,
            },
        );

        return seek_result;
    }
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            // Use PreviousSync for better compatibility - seeks to nearest keyframe before target
            // Closest mode might have issues on some devices/video formats
            // Clamp to i32 range for AVPlayer
            let pos_i32 = position_ms.clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            return p
                .seek(
                    pos_i32,
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
    if lingxia_platform::harmony::video_player::has_stream_decoder(&component_id) {
        return lingxia_platform::harmony::video_player::set_stream_volume(
            &component_id,
            volume as f32,
        )
        .is_ok();
    }
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

/// Set playback speed
#[napi]
pub fn video_player_set_speed(component_id: String, rate: f64) -> bool {
    lingxia_platform::harmony::video_player::set_speed_from_rate(&component_id, rate).is_ok()
}

/// Get current playback position in milliseconds
#[napi]
pub fn video_player_get_current_time(component_id: String) -> i32 {
    // Try native AVPlayer first (for URL/file playback)
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(mut p) = player.lock() {
            if let Ok(position) = p.get_current_time() {
                // Return AVPlayer position if valid (>= 0), including 0 which is a valid time
                return position;
            }
        }
    }
    // Fallback to stream decoder position (for stream mode)
    lingxia_platform::harmony::video_player::get_stream_decoder_position_ms(&component_id)
        .unwrap_or(0)
}

/// Get duration in milliseconds
#[napi]
pub fn video_player_get_duration(component_id: String) -> i32 {
    if let Some(player) = lingxia_platform::harmony::video_player::get_player(&component_id) {
        if let Ok(p) = player.lock() {
            let duration = p.get_duration().unwrap_or(0);
            if duration > 0 {
                return duration;
            }
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
    let _ = lingxia_platform::harmony::video_player::dispatch_command(
        &component_id,
        VideoPlayerCommand::Stop,
    );
    lingxia_platform::harmony::video_player::destroy_player(&component_id).is_ok()
}

/// Notify that app entered foreground
/// Called from LingxiaBaseAbility.onForeground
#[napi]
pub fn on_app_show(lxappid: String) {
    if let Some(lxapp) = lxapp::try_get(&lxappid) {
        let args = lxapp::AppServiceEventArgs {
            source: lxapp::AppServiceEventSource::Host,
            reason: lxapp::AppServiceEventReason::Foreground,
        }
        .to_json_string();
        let _ = lxapp.appservice_notify(lxapp::AppServiceEvent::OnShow, Some(args));
    }
}

/// Notify that app entered background
/// Called from LingxiaBaseAbility.onBackground
#[napi]
pub fn on_app_hide(lxappid: String) {
    if let Some(lxapp) = lxapp::try_get(&lxappid) {
        let args = lxapp::AppServiceEventArgs {
            source: lxapp::AppServiceEventSource::Host,
            reason: lxapp::AppServiceEventReason::Background,
        }
        .to_json_string();
        let _ = lxapp.appservice_notify(lxapp::AppServiceEvent::OnHide, Some(args));
    }
}

/// Notify that user captured a screenshot
#[napi]
pub fn on_user_capture_screen(lxappid: String) {
    if let Some(lxapp) = lxapp::try_get(&lxappid) {
        let args = lxapp::AppServiceEventArgs {
            source: lxapp::AppServiceEventSource::Host,
            reason: lxapp::AppServiceEventReason::Screenshot,
        }
        .to_json_string();
        let _ = lxapp.appservice_notify(lxapp::AppServiceEvent::OnUserCaptureScreen, Some(args));
    }
}
