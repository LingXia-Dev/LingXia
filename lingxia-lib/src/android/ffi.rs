use crate::{App, runtime::PlatformAppRuntime};
use android_logger::Config;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jint};
use jni::{JNIEnv, JavaVM};
use lingxia_webview::init_webview_manager;
use log::{error, info};
use lxapp::{LxAppDelegate, log::LogLevel};

/// Parses a color string (e.g., "#RRGGBB" or "transparent") into an i32 ARGB value for Android.
fn parse_color_to_i32(color_str: &str, default_color: i32) -> i32 {
    if color_str.eq_ignore_ascii_case("transparent") {
        return 0x00000000;
    }

    if color_str.starts_with('#') && color_str.len() == 7 {
        if let Ok(rgb) = i32::from_str_radix(&color_str[1..], 16) {
            return (0xFF000000u32 as i32) | rgb; // Add full alpha
        }
    }

    default_color
}

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::os::raw::c_void) -> jint {
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("Rust"),
    );

    // Initialize the new logging system
    lxapp::log::LogManager::init(|log_message| {
        let formatted_message = format!(
            "[{}{}{}] {}",
            log_message.tag.as_str(),
            log_message
                .appid
                .as_ref()
                .map(|id| format!(":{}", id))
                .unwrap_or_default(),
            log_message
                .path
                .as_ref()
                .map(|p| format!(":{}", p))
                .unwrap_or_default(),
            log_message.message
        );

        match log_message.level {
            LogLevel::Verbose => log::trace!("{}", formatted_message),
            LogLevel::Debug => log::debug!("{}", formatted_message),
            LogLevel::Info => log::info!("{}", formatted_message),
            LogLevel::Warn => log::warn!("{}", formatted_message),
            LogLevel::Error => log::error!("{}", formatted_message),
        }
    });

    // Create global reference to LxApp class for worker threads first
    if let Ok(mut env) = vm.get_env() {
        if let Ok(local_class) = env.find_class("com/lingxia/lxapp/LxApp") {
            if let Ok(global_class) = env.new_global_ref(local_class) {
                super::app::init_lxapp_class(global_class);
            }
        }
    }

    // Initialize JNI environment
    lingxia_webview::initialize_jni(vm);

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppInited(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    cache_dir: JString,
    asset_manager: JObject,
) -> jni::sys::jstring {
    let data_dir: String = env.get_string(&data_dir).unwrap().into();
    let cache_dir: String = env.get_string(&cache_dir).unwrap().into();

    log::info!(
        "Initializing LxApp with data_dir: {}, cache_dir: {}",
        data_dir,
        cache_dir,
    );

    let app = match App::from_java(&mut env, asset_manager.as_raw(), data_dir, cache_dir) {
        Ok(app) => app,
        Err(_) => {
            return JObject::null().into_raw();
        }
    };

    // Initialize WebView manager
    init_webview_manager();

    // Initialize platform runtime and lxapp
    let runtime = PlatformAppRuntime::init(app);
    let home_app_id = lxapp::init(runtime);

    // Return the home appid
    match home_app_id {
        Some(appid) => match env.new_string(&appid) {
            Ok(java_string) => java_string.into_raw(),
            Err(_) => JObject::null().into_raw(),
        },
        None => {
            error!("Failed to obtain LxApp home app details during initialization.");
            JObject::null().into_raw()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onPageShow(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    path: JString,
) {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let lxapp = lxapp::get(appid);
    lxapp.on_page_show(path);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_findWebView<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    if let Some(webview) = lingxia_webview::find_webview(&appid, &path) {
        // Get direct access to the WebView and create a new local reference to the Java WebView object
        match env.new_local_ref(webview.get_java_webview()) {
            Ok(local_ref) => unsafe { JObject::from_raw(local_ref.into_raw()) },
            Err(e) => {
                error!("Failed to create local reference to WebView: {:?}", e);
                JObject::null()
            }
        }
    } else {
        // No WebView found for this appid/path
        error!("💥 Not found webview for {}-{}", appid, path);
        JObject::null()
    }
}

// Function for LxAppActivity class to handle the mini app close event
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppClosed(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();

    let lxapp = lxapp::get(appid.clone());
    lxapp.on_lxapp_closed();
    0
}

/// Get navigation bar configuration for a specific page
///
/// IMPORTANT: This function returns NavigationBarConfig directly to Kotlin.
/// Kotlin side should handle visibility logic based on:
/// - navigationStyle: 0=Default (show), 1=Custom (hide)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getNavigationBarConfig<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the lxapp instance
    let lxapp = lxapp::get(appid.clone());

    // Get navigation bar config using new API
    let nav_config = lxapp.get_config().get_nav_bar_config(&lxapp, &path);

    // Debug logging
    log::info!(
        "[Android] nativeGetPageConfig: appid={}, path={}, navigationStyle={:?}, bgColor={}, textStyle={}, title={}",
        appid,
        path,
        nav_config.navigationStyle,
        nav_config.navigationBarBackgroundColor,
        nav_config.navigationBarTextStyle,
        nav_config.navigationBarTitleText
    );

    // Find the NavigationBarConfig class
    let nav_bar_class = match env.find_class("com/lingxia/lxapp/NavigationBarConfig") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Parse background color using unified function
    let bg_color_int = parse_color_to_i32(&nav_config.navigationBarBackgroundColor, 0xFFFFFFFFu32 as i32);

    log::info!(
        "[Android] Color parsing: original={}, parsed=0x{:08X}",
        nav_config.navigationBarBackgroundColor,
        bg_color_int as u32
    );

    // Create Java strings
    let title_text = match env.new_string(&nav_config.navigationBarTitleText) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let text_style = match env.new_string(&nav_config.navigationBarTextStyle) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    // Use int for navigation style (0=Default, 1=Custom)
    let navigation_style_int = nav_config.navigationStyle.to_i32();

    // Create NavigationBarConfig object (using int for navigation style)
    match env.new_object(
        nav_bar_class,
        "(ILjava/lang/String;Ljava/lang/String;I)V",
        &[
            (bg_color_int as jint).into(),
            (&text_style).into(),
            (&title_text).into(),
            (navigation_style_int as jint).into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_lxapp_NativeApi_onBackPressed(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let lxapp = lxapp::get(appid);
    if lxapp.on_back_pressed() { 1 } else { 0 }
}

// Function to notify the Rust layer that a mini app has been opened
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppOpened(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    path: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let lxapp = lxapp::get(appid.clone());
    lxapp.on_lxapp_opened(path);
    0
}

/// Get LxApp information using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getLxAppInfo<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let lxapp = lxapp::get(appid.clone());

    let lxapp_info = lxapp.get_config().get_lxapp_info();

    // Debug logging
    log::info!(
        "[Android] nativeGetLxAppInfo: appid={}, initial_route={}, app_name={}, debug={}",
        appid,
        lxapp_info.initial_route,
        lxapp_info.app_name,
        lxapp_info.debug
    );

    // Find the LxAppInfo class
    let lxapp_info_class = match env.find_class("com/lingxia/lxapp/LxAppInfo") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Create Java strings
    let initial_route_str = match env.new_string(&lxapp_info.initial_route) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let app_name_str = match env.new_string(&lxapp_info.app_name) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };

    // Create LxAppInfo object
    match env.new_object(
        lxapp_info_class,
        "(Ljava/lang/String;Ljava/lang/String;Z)V",
        &[
            (&initial_route_str).into(),
            (&app_name_str).into(),
            (lxapp_info.debug as jboolean).into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

// Get TabBar configuration using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getTabBarConfig<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let lxapp = lxapp::get(appid.clone());

    let tab_bar_config = match lxapp.get_config().get_tab_bar_config(&lxapp) {
        Some(config) => config,
        None => {
            log::info!(
                "[Android] nativeGetTabBarConfig: No TabBar config found for appid={}",
                appid
            );
            return JObject::null();
        }
    };

    // Debug logging
    log::info!(
        "[Android] nativeGetTabBarConfig: appid={}, items_count={}",
        appid,
        tab_bar_config.list.len()
    );

    // Find the TabBarConfig class
    let tab_bar_class = match env.find_class("com/lingxia/lxapp/TabBarConfig") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Convert background color using unified function
    let background_color = parse_color_to_i32(&tab_bar_config.backgroundColor, 0xFFFFFFFFu32 as i32);

    // Convert selected color using unified function
    let selected_color = parse_color_to_i32(&tab_bar_config.selectedColor, 0xFF1677FFu32 as i32);

    // Convert unselected color using unified function
    let color = parse_color_to_i32(&tab_bar_config.color, 0xFF666666u32 as i32);

    // Convert border style using unified function
    let border_style = parse_color_to_i32(&tab_bar_config.borderStyle, 0xFFF0F0F0u32 as i32);

    // Convert dimension (height for top/bottom, width for left/right)
    let dimension = tab_bar_config.dimension;

    // Use int for position (0=Bottom, 1=Top, 2=Left, 3=Right)
    let position_int = tab_bar_config.position.to_i32();

    // Create TabBarItem list
    let array_list_class = match env.find_class("java/util/ArrayList") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    let tab_items_list = match env.new_object(array_list_class, "()V", &[]) {
        Ok(list) => list,
        Err(_) => return JObject::null(),
    };

    // Add TabBarItems to the list
    for item in &tab_bar_config.list {
        if let Some(tab_item) = create_tab_bar_item(&mut env, item) {
            let _ = env.call_method(
                &tab_items_list,
                "add",
                "(Ljava/lang/Object;)Z",
                &[(&tab_item).into()],
            );
        }
    }

    // Create Position enum
    let position_class = match env.find_class("com/lingxia/lxapp/TabBarConfig$Position") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    let position_enum_value = match position_int {
        1 => "LEFT",
        2 => "RIGHT",
        _ => "BOTTOM", // default
    };

    let position_enum = match env.get_static_field(
        position_class,
        position_enum_value,
        "Lcom/lingxia/lxapp/TabBarConfig$Position;",
    ) {
        Ok(pos) => pos,
        Err(_) => return JObject::null(),
    };

    // Create TabBarConfig object (all parameters non-nullable)
    match env.new_object(
        tab_bar_class,
        "(IIIIILcom/lingxia/lxapp/TabBarConfig$Position;Ljava/util/List;Z)V",
        &[
            background_color.into(),
            selected_color.into(),
            color.into(),
            border_style.into(),
            dimension.into(),
            (&position_enum).into(),
            (&tab_items_list).into(),
            true.into(), // visible
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

// Helper function to create TabBarItem
fn create_tab_bar_item<'a>(
    env: &mut JNIEnv<'a>,
    item: &lxapp::config::TabItem,
) -> Option<JObject<'a>> {
    let tab_bar_item_class = env.find_class("com/lingxia/lxapp/TabBarItem").ok()?;

    let page_path = env.new_string(&item.pagePath).ok()?;
    let text = if let Some(ref text_str) = item.text {
        env.new_string(text_str).ok()
    } else {
        None
    };
    let icon_path = if let Some(ref icon_str) = item.iconPath {
        env.new_string(icon_str).ok()
    } else {
        env.new_string("").ok()
    }?;
    let selected_icon_path = if let Some(ref selected_icon_str) = item.selectedIconPath {
        env.new_string(selected_icon_str).ok()
    } else {
        env.new_string("").ok()
    }?;

    // Group positioning: 0=middle/center (default), 1=start (top/left), 2=end (bottom/right)
    let group = match &item.group {
        Some(lxapp::config::TabItemGroup::Start) => 1i32,
        Some(lxapp::config::TabItemGroup::End) => 2i32,
        None => 0i32,
    };

    let text_obj = text.map(|t| t.into()).unwrap_or_else(|| JObject::null());

    env.new_object(
        tab_bar_item_class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZI)V",
        &[
            (&page_path).into(),
            (&text_obj).into(),
            (&icon_path).into(),
            (&selected_icon_path).into(),
            item.selected.into(),
            true.into(), // visible - TabItem doesn't have visible field, default to true
            group.into(),
        ],
    )
    .ok()
}

/// Get TabBar item by index
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getTabBarItem<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    index: jint,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let lxapp = lxapp::get(appid.clone());

    let tab_bar_config = match lxapp.get_config().get_tab_bar_config(&lxapp) {
        Some(config) => config,
        None => {
            log::info!(
                "[Android] nativeGetTabBarItem: No TabBar config found for appid={}",
                appid
            );
            return JObject::null();
        }
    };

    let item = match tab_bar_config.list.get(index as usize) {
        Some(item) => item,
        None => {
            log::info!(
                "[Android] nativeGetTabBarItem: Index {} out of bounds for appid={}",
                index,
                appid
            );
            return JObject::null();
        }
    };

    // Debug logging
    log::info!(
        "[Android] nativeGetTabBarItem: appid={}, index={}, page_path={}",
        appid,
        index,
        item.pagePath
    );

    create_tab_bar_item(&mut env, item).unwrap_or(JObject::null())
}
