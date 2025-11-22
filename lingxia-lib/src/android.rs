use android_logger::Config;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::{JNIEnv, JavaVM};
use lingxia_messaging::invoke_callback;
use lingxia_platform::CachedClass;
use log::{error, info};
use lxapp::{LxAppDelegate, UiEventType, log::LogLevel};

/// Parses a color string (e.g., "#RRGGBB" or "transparent") into an i32 ARGB value for Android.
fn parse_color_to_i32(color_str: &str, default_color: i32) -> i32 {
    if color_str.eq_ignore_ascii_case("transparent") {
        return 0x00000000;
    }

    if color_str.starts_with('#')
        && color_str.len() == 7
        && let Ok(rgb) = i32::from_str_radix(&color_str[1..], 16)
    {
        return (0xFF000000u32 as i32) | rgb; // Add full alpha
    }

    default_color
}

fn init_cached_java_class(env: &mut JNIEnv<'_>, class: CachedClass) {
    if let Ok(local_class) = env.find_class(class.class_path())
        && let Ok(global_class) = env.new_global_ref(local_class)
    {
        lingxia_platform::init_cached_class(class, global_class);
    }
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
        init_cached_java_class(&mut env, CachedClass::LxApp);
        init_cached_java_class(&mut env, CachedClass::PreviewMediaPayload);
        init_cached_java_class(&mut env, CachedClass::LxAppMedia);
        init_cached_java_class(&mut env, CachedClass::LxAppDevice);
        init_cached_java_class(&mut env, CachedClass::LxAppLocation);
        init_cached_java_class(&mut env, CachedClass::LxAppPopup);
        init_cached_java_class(&mut env, CachedClass::LxAppToast);
        init_cached_java_class(&mut env, CachedClass::LxAppModal);
        init_cached_java_class(&mut env, CachedClass::LxAppActionSheet);
        init_cached_java_class(&mut env, CachedClass::LxAppPicker);
        init_cached_java_class(&mut env, CachedClass::LxAppDocument);
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
    locale: JString,
) -> jni::sys::jstring {
    let data_dir: String = env.get_string(&data_dir).unwrap().into();
    let cache_dir: String = env.get_string(&cache_dir).unwrap().into();
    let locale: String = env.get_string(&locale).unwrap().into();

    log::info!(
        "Initializing LxApp with data_dir: {}, cache_dir: {}, locale: {}",
        data_dir,
        cache_dir,
        locale
    );

    let app = match unsafe {
        lingxia_platform::Platform::from_java(
            &mut env,
            asset_manager.as_raw(),
            data_dir,
            cache_dir,
            locale,
        )
    } {
        Ok(app) => app,
        Err(_) => {
            return JObject::null().into_raw();
        }
    };

    lingxia_logic::register_logic_runtime();
    let home_app_id = lxapp::init(app);

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

    let webtag = lingxia_webview::WebTag::new(&appid, &path, None);
    if let Some(webview) = lingxia_webview::find_webview(&webtag) {
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
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getNavigationBarState<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the lxapp instance
    let lxapp = lxapp::get(appid.clone());

    // Get navigation bar state using new API
    let nav_state = lxapp.get_navbar_state(&path);

    // Find the NavigationBarState class
    let nav_bar_class = match env.find_class("com/lingxia/lxapp/NavigationBarState") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Parse background color using unified function
    let bg_color_int = parse_color_to_i32(
        &nav_state.navigationBarBackgroundColor,
        0xFFFFFFFFu32 as i32,
    );

    // Create Java strings
    let title_text = match env.new_string(&nav_state.navigationBarTitleText) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let text_style = match env.new_string(&nav_state.navigationBarTextStyle) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };

    // Create NavigationBarState object with new boolean fields
    // Constructor signature: (ILjava/lang/String;Ljava/lang/String;ZZZ)V
    // Parameters: backgroundColor, textStyle, titleText, showNavbar, showBackButton, showHomeButton
    match env.new_object(
        nav_bar_class,
        "(ILjava/lang/String;Ljava/lang/String;ZZZ)V",
        &[
            (bg_color_int as jint).into(),
            (&text_style).into(),
            (&title_text).into(),
            (nav_state.show_navbar as jboolean).into(),
            (nav_state.show_back_button as jboolean).into(),
            (nav_state.show_home_button as jboolean).into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_lxapp_NativeApi_onUiEvent(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    event_type: jint,
    data: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let data_str: String = env.get_string(&data).unwrap().into();

    let ui_event_type = match event_type {
        0 => UiEventType::TabBarClick,
        1 => UiEventType::CapsuleClick,
        2 => UiEventType::NavigationClick,
        3 => UiEventType::BackPress,
        _ => {
            error!("Unknown UI event type: {}", event_type);
            return 0;
        }
    };

    let lxapp = lxapp::get(appid);
    if lxapp.on_ui_event(ui_event_type, data_str) {
        1
    } else {
        0
    }
}

// Function to notify the Rust layer that a mini app has been opened
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppOpened<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JString<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let lxapp = lxapp::get(appid.clone());
    let resolved_path = lxapp.on_lxapp_opened(path);

    match env.new_string(&resolved_path) {
        Ok(jstring) => jstring,
        Err(_) => {
            // Return empty string as fallback
            env.new_string("").unwrap_or_else(|_| {
                // If even empty string fails, return null
                JString::from(jni::objects::JObject::null())
            })
        }
    }
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

    let lxapp_info = lxapp.get_lxapp_info();

    // Find the LxAppInfo class
    let lxapp_info_class = match env.find_class("com/lingxia/lxapp/LxAppInfo") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Create Java strings
    let app_name_str = match env.new_string(&lxapp_info.app_name) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let cache_dir_str = match env.new_string(lxapp.user_cache_dir.to_string_lossy().into_owned()) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };

    // Create LxAppInfo object (appName, cacheDir)
    match env.new_object(
        lxapp_info_class,
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[(&app_name_str).into(), (&cache_dir_str).into()],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

// Get TabBar configuration using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getTabBarState<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let lxapp = lxapp::get(appid.clone());

    let tab_bar_config = match lxapp.get_tabbar() {
        Some(config) => config,
        None => {
            return JObject::null();
        }
    };

    // Find the TabBarState class
    let tab_bar_class = match env.find_class("com/lingxia/lxapp/TabBarState") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Convert background color using unified function
    let background_color =
        parse_color_to_i32(&tab_bar_config.backgroundColor, 0xFFFFFFFFu32 as i32);

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

    for item in tab_bar_config.list.iter() {
        if let Some(tab_item) = create_tab_bar_item(&mut env, item) {
            let _ = env.call_method(
                &tab_items_list,
                "add",
                "(Ljava/lang/Object;)Z",
                &[(&tab_item).into()],
            );
        } else {
            log::warn!(
                "[Android] Failed to create TabBar item in getTabBarState for {}",
                &item.pagePath
            );
        }
    }

    // Create Position enum
    let position_class = match env.find_class("com/lingxia/lxapp/TabBarState$Position") {
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
        "Lcom/lingxia/lxapp/TabBarState$Position;",
    ) {
        Ok(pos) => pos,
        Err(_) => return JObject::null(),
    };

    // Create TabBarState object (all parameters non-nullable)
    match env.new_object(
        tab_bar_class,
        "(IIIIILcom/lingxia/lxapp/TabBarState$Position;Ljava/util/List;ZI)V",
        &[
            background_color.into(),
            selected_color.into(),
            color.into(),
            border_style.into(),
            dimension.into(),
            (&position_enum).into(),
            (&tab_items_list).into(),
            tab_bar_config.is_visible.into(),
            tab_bar_config.selected_index.into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

/// Create TabBarItem with actual badge and red dot data from Rust
fn create_tab_bar_item<'a>(
    env: &mut JNIEnv<'a>,
    item: &lxapp::tabbar::TabBarItem,
) -> Option<JObject<'a>> {
    // Find TabBarItem class
    let tab_item_class = match env.find_class("com/lingxia/lxapp/TabBarItem") {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Convert group enum
    let group_int = match &item.group {
        Some(lxapp::tabbar::TabItemGroup::Start) => 1,
        Some(lxapp::tabbar::TabItemGroup::End) => 2,
        None => 0,
    };

    // Create strings
    let page_path = match env.new_string(&item.pagePath) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let text = match env.new_string(item.text.as_deref().unwrap_or("")) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let icon_path = match env.new_string(item.iconPath.as_deref().unwrap_or("")) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let selected_icon_path = match env.new_string(item.selectedIconPath.as_deref().unwrap_or("")) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // Create badge string from actual Rust data (nullable)
    let badge_jstring = match &item.badge {
        Some(badge) => match env.new_string(badge) {
            Ok(s) => s.into(),
            Err(_) => JObject::null(),
        },
        None => JObject::null(),
    };

    // Create TabBarItem object with actual data
    env
        .new_object(
            tab_item_class,
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/String;Z)V",
            &[
                (&page_path).into(),
                (&text).into(),
                (&icon_path).into(),
                (&selected_icon_path).into(),
                item.selected.into(),
                group_int.into(),
                (&badge_jstring).into(),
                item.has_red_dot.into(), // Use actual red dot data from Rust
            ],
        )
        .ok()
}

/// Handle DeepLink URL by processing the path without host
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onAppLinkReceived(
    mut env: JNIEnv,
    _class: JClass,
    applink_url: JString,
) -> jint {
    let url: String = env.get_string(&applink_url).unwrap().into();

    log::info!("[Android] AppLink received: {}", url);
    0
}

/// Get current active LxApp ID and path from Rust stack
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getCurrentLxApp<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
) -> JObject<'a> {
    let (current_appid, current_path) = lxapp::get_current_lxapp();

    // Find the CurrentLxApp class (we'll need to create this)
    let current_lxapp_class = match env.find_class("com/lingxia/lxapp/CurrentLxApp") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Create Java strings
    let appid_str = match env.new_string(&current_appid) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let path_str = match env.new_string(&current_path) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };

    // Create CurrentLxApp object
    match env.new_object(
        current_lxapp_class,
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[(&appid_str).into(), (&path_str).into()],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

/// Callback from platform (called from Kotlin via NativeAPI)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onCallback(
    mut env: JNIEnv,
    _class: JClass,
    id: jlong,
    success: jboolean,
    data: JString,
) -> jboolean {
    let id = id as u64;
    let success = success != 0;

    let data_str: String = match env.get_string(&data) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("[Android] Failed to get data string: {}", e);
            return 0;
        }
    };

    if invoke_callback(id, success, data_str) {
        1
    } else {
        0
    }
}
