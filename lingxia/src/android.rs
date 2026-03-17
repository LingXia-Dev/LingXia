use android_logger::Config;
use jni::objects::{JClass, JObject, JString};
use jni::strings::JNIString;
use jni::sys::{jboolean, jint, jlong};
use jni::{
    Env, EnvUnowned, JavaVM,
    errors::{LogErrorAndDefault, ThrowRuntimeExAndDefault},
    jni_sig, jni_str,
};
use lingxia_messaging::invoke_callback;
use lingxia_platform::CachedClass;
use log::{error, info, warn};
use lxapp::{
    AppServiceEvent, AppServiceEventArgs, AppServiceEventReason, AppServiceEventSource,
    LxAppDelegate, LxAppUiEventType, OrientationConfig, PageOrientation, log::LogLevel,
};

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

fn init_cached_java_class(env: &mut Env<'_>, class: CachedClass) {
    match env.find_class(JNIString::new(class.class_path())) {
        Ok(local_class) => match env.new_global_ref(local_class) {
            Ok(global_class) => lingxia_platform::init_cached_class(class, global_class),
            Err(e) => warn!(
                "Failed to create global ref for cached class {}: {:?}",
                class.class_path(),
                e
            ),
        },
        Err(e) => {
            // `FindClass` leaves a pending exception. We treat this as best-effort caching,
            // so clear it to keep JNI usable.
            let _ = env.exception_clear();
            warn!(
                "Failed to find cached class {} (will retry later): {:?}",
                class.class_path(),
                e
            );
        }
    }
}

fn init_cached_java_classes(env: &mut Env<'_>) {
    // Keep this in sync with `lingxia_platform::CachedClass`.
    let classes = [
        CachedClass::LxApp,
        CachedClass::PreviewMediaPayload,
        CachedClass::LxAppMedia,
        CachedClass::LxAppDevice,
        CachedClass::LxAppLocation,
        CachedClass::LxAppPopup,
        CachedClass::LxAppToast,
        CachedClass::LxAppModal,
        CachedClass::LxAppActionSheet,
        CachedClass::LxAppPicker,
        CachedClass::LxAppDocument,
        CachedClass::ComponentRouter,
        CachedClass::LxAppPullToRefresh,
        CachedClass::UpdateManager,
        CachedClass::LxAppCapsule,
        CachedClass::LxAppWifi,
        CachedClass::LxAppNetwork,
    ];

    for class in classes {
        init_cached_java_class(env, class);
    }
}

#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
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

    // Only store JavaVM here. App/library classes must be cached from a Java->native call so
    // `FindClass` uses the correct classloader.
    lingxia_webview::platform::android::initialize_jni(vm);

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppInited<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    data_dir: JString<'a>,
    cache_dir: JString<'a>,
    asset_manager: JObject<'a>,
    locale: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        // Cache app/library classes here (Java -> native entrypoint) so `FindClass` resolves via
        // the app classloader. Doing this in `JNI_OnLoad` can fail on Android.
        init_cached_java_classes(env);

        let data_dir_str: String = data_dir.try_to_string(env)?;
        let cache_dir_str: String = cache_dir.try_to_string(env)?;
        let locale_str: String = locale.try_to_string(env)?;

        log::info!(
            "Initializing LxApp with data_dir: {}, cache_dir: {}, locale: {}",
            data_dir_str,
            cache_dir_str,
            locale_str
        );

        let platform = unsafe {
            lingxia_platform::Platform::from_java(
                env,
                asset_manager.as_raw(),
                data_dir_str,
                cache_dir_str,
                locale_str,
            )
        }
        .map_err(|_| jni::errors::Error::JniCall(jni::errors::JniError::Unknown))?;

        let home_app_id = crate::init_with_platform(platform);

        // Return the home appid
        match home_app_id {
            Some(appid) => {
                let java_string = env.new_string(&appid)?;
                Ok(java_string)
            }
            None => {
                error!("Failed to obtain LxApp home app details during initialization.");
                Ok(JString::null())
            }
        }
    })
    .resolve::<LogErrorAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onPageShow(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    path: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;

        if let Some(lxapp) = lxapp::try_get(&appid) {
            lxapp.on_page_show(path);
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_findWebView<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
    session_id: jlong,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;

        if session_id <= 0 {
            warn!(
                "findWebView called without valid session_id for {}:{}",
                appid, path
            );
            return Ok(JObject::null());
        }
        let session = Some(session_id as u64);
        let webtag = lingxia_webview::WebTag::new(&appid, &path, session);
        if let Some(webview) = lingxia_webview::runtime::find_webview(&webtag) {
            // Get direct access to the WebView and create a new local reference to the Java WebView object
            match env.new_local_ref(webview.get_java_webview()) {
                Ok(local_ref) => Ok(unsafe { JObject::from_raw(env, local_ref.into_raw()) }),
                Err(e) => {
                    error!("Failed to create local reference to WebView: {:?}", e);
                    Ok(JObject::null())
                }
            }
        } else {
            // No WebView found for this appid/path
            error!(
                "💥 Not found webview for {}-{} (session={}, key={})",
                appid,
                path,
                session_id,
                webtag.key()
            );
            Ok(JObject::null())
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// Function for LxAppActivity class to handle the mini app close event
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppClosed(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    session_id: jlong,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let Some(lxapp) = lxapp::try_get(&appid) else {
            warn!("Received close event for unknown lxapp: {}", appid);
            return Ok(false);
        };
        if session_id <= 0 {
            warn!("Ignoring close event with invalid session_id for {}", appid);
            return Ok(false);
        }
        let session_id = session_id as u64;
        if session_id != lxapp.session_id() {
            return Ok(false);
        }
        lxapp.on_lxapp_closed(session_id);
        Ok(true)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Get navigation bar configuration for a specific page
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getNavigationBarState<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;

        // Get the lxapp instance
        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(JObject::null());
        };

        // Get navigation bar state from page
        let nav_state = lxapp
            .get_page(&path)
            .and_then(|page| page.get_navbar_state())
            .unwrap_or_default();

        // Find the NavigationBarState class
        let nav_bar_class = env.find_class(jni_str!("com/lingxia/lxapp/NavigationBarState"))?;

        // Parse background color using unified function
        let bg_color_int = parse_color_to_i32(
            &nav_state.navigationBarBackgroundColor,
            0xFFFFFFFFu32 as i32,
        );

        // Create Java strings
        let title_text = env.new_string(&nav_state.navigationBarTitleText)?;
        let text_style = env.new_string(&nav_state.navigationBarTextStyle)?;

        // Create NavigationBarState object with new boolean fields
        // Constructor signature: (ILjava/lang/String;Ljava/lang/String;ZZZ)V
        // Parameters: backgroundColor, textStyle, titleText, showNavbar, showBackButton, showHomeButton
        let obj = env.new_object(
            nav_bar_class,
            jni_sig!("(ILjava/lang/String;Ljava/lang/String;ZZZ)V"),
            &[
                (bg_color_int as jint).into(),
                (&text_style).into(),
                (&title_text).into(),
                (nav_state.show_navbar as jboolean).into(),
                (nav_state.show_back_button as jboolean).into(),
                (nav_state.show_home_button as jboolean).into(),
            ],
        )?;
        Ok(obj)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Check if pull-to-refresh is enabled for a specific page
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_isPullDownRefreshEnabled<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;

        if lxapp::is_pull_down_refresh_enabled(&appid, &path) {
            Ok(true)
        } else {
            Ok(false)
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Get page orientation for a specific page
/// Returns: 0=auto, 1=portrait, 2=landscape, 3=reverse-portrait, 4=reverse-landscape
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getPageOrientation<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;

        let Some(lxapp_instance) = lxapp::try_get(&appid) else {
            return Ok(0);
        };

        let orientation = lxapp_instance.get_page_orientation(&path);
        Ok(orientation_to_android_value(orientation))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

fn orientation_to_android_value(orientation: OrientationConfig) -> jint {
    match (orientation.mode, orientation.rotation) {
        (PageOrientation::Auto, _) => 0,
        (PageOrientation::Portrait, 180) => 3,
        (PageOrientation::Portrait, _) => 1,
        (PageOrientation::Landscape, 180) => 4,
        (PageOrientation::Landscape, _) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_lxapp_NativeApi_onLxappEvent(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    event_type: jint,
    data: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let data_str: String = data.try_to_string(env)?;

        let ui_event_type = match event_type {
            0 => LxAppUiEventType::TabBarClick,
            1 => LxAppUiEventType::CapsuleClick,
            2 => LxAppUiEventType::NavigationClick,
            3 => LxAppUiEventType::BackPress,
            4 => LxAppUiEventType::PullDownRefresh,
            _ => {
                error!("Unknown UI event type: {}", event_type);
                return Ok(0);
            }
        };

        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(0);
        };
        if lxapp.on_lxapp_event(ui_event_type, data_str) {
            Ok(1)
        } else {
            Ok(0)
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_lxapp_NativeApi_onKeyEvent(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    event_type: jint,
    payload_json: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let payload: String = payload_json.try_to_string(env)?;

        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(false);
        };
        let session_id = lxapp.session_id();

        const KEY_EVENT_DOWN: jint = 0;
        const KEY_EVENT_UP: jint = 1;

        let should_dispatch = match event_type {
            KEY_EVENT_DOWN => lxapp::key_event::has_key_down(&appid, session_id),
            KEY_EVENT_UP => lxapp::key_event::has_key_up(&appid, session_id),
            _ => false,
        };

        if !should_dispatch {
            return Ok(false);
        }

        let event_name = if event_type == KEY_EVENT_DOWN {
            "KeyDown"
        } else {
            "KeyUp"
        };
        if lxapp::publish_app_event(&appid, event_name, Some(payload)) {
            Ok(true)
        } else {
            Ok(false)
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_lxapp_NativeApi_onDeviceOrientationChanged(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    session_id: jlong,
    value: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let value: String = value.try_to_string(env)?;

        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(false);
        };

        if session_id <= 0 {
            return Ok(false);
        }
        if lxapp.session_id() != session_id as u64 {
            return Ok(false);
        }

        let normalized = match value.as_str() {
            "portrait" => "portrait",
            "landscape" => "landscape",
            _ => return Ok(false),
        };

        let payload = format!(r#"{{"value":"{}"}}"#, normalized);
        if lxapp::publish_app_event(&appid, "DeviceOrientationChange", Some(payload)) {
            Ok(true)
        } else {
            Ok(false)
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// Function to notify the Rust layer that a mini app has been opened
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppOpened<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
    session_id: jlong,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        if session_id <= 0 {
            warn!(
                "onLxAppOpened called without valid session_id for {}",
                appid
            );
            return env.new_string("");
        }
        let resolved_path = lxapp::try_get(&appid)
            .map(|lxapp| lxapp.on_lxapp_opened(path, session_id as u64))
            .unwrap_or_default();

        match env.new_string(&resolved_path) {
            Ok(jstring) => Ok(jstring),
            Err(_) => {
                // Return empty string as fallback
                env.new_string("").or_else(|_| {
                    // If even empty string fails, return null
                    Ok(JString::null())
                })
            }
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Get LxApp information using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getLxAppInfo<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(JObject::null());
        };

        let lxapp_info = lxapp.get_lxapp_info();

        // Find the LxAppInfo class
        let lxapp_info_class = env.find_class(jni_str!("com/lingxia/lxapp/LxAppInfo"))?;

        // Create Java strings
        let app_name_str = env.new_string(&lxapp_info.app_name)?;
        let version_str = env.new_string(&lxapp_info.version)?;
        let release_type_str = env.new_string(&lxapp_info.release_type)?;
        let cache_dir_str = env.new_string(lxapp.user_cache_dir.to_string_lossy().into_owned())?;

        // Create LxAppInfo object (appName, version, releaseType, cacheDir)
        let obj = env.new_object(
            lxapp_info_class,
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
            &[
                (&app_name_str).into(),
                (&version_str).into(),
                (&release_type_str).into(),
                (&cache_dir_str).into(),
            ],
        )?;
        Ok(obj)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// Get TabBar configuration using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getTabBarState<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;

        let tab_bar_config = match lxapp::try_get(&appid).and_then(|lxapp| lxapp.get_tabbar()) {
            Some(config) => config,
            None => {
                return Ok(JObject::null());
            }
        };

        // Find the TabBarState class
        let tab_bar_class = env.find_class(jni_str!("com/lingxia/lxapp/TabBarState"))?;

        // Convert background color using unified function
        let background_color =
            parse_color_to_i32(&tab_bar_config.backgroundColor, 0xFFFFFFFFu32 as i32);

        // Convert selected color using unified function
        let selected_color =
            parse_color_to_i32(&tab_bar_config.selectedColor, 0xFF1677FFu32 as i32);

        // Convert unselected color using unified function
        let color = parse_color_to_i32(&tab_bar_config.color, 0xFF666666u32 as i32);

        // Convert border style using unified function
        let border_style = parse_color_to_i32(&tab_bar_config.borderStyle, 0xFFF0F0F0u32 as i32);

        // Convert dimension (height for top/bottom, width for left/right)
        let dimension = tab_bar_config.dimension;

        // Use int for position (0=Bottom, 1=Top, 2=Left, 3=Right)
        let position_int = tab_bar_config.position.to_i32();

        // Create TabBarItem list
        let array_list_class = env.find_class(jni_str!("java/util/ArrayList"))?;

        let tab_items_list = env.new_object(array_list_class, jni_sig!("()V"), &[])?;

        for item in tab_bar_config.list.iter() {
            if let Some(tab_item) = create_tab_bar_item(env, item) {
                let _ = env.call_method(
                    &tab_items_list,
                    jni_str!("add"),
                    jni_sig!("(Ljava/lang/Object;)Z"),
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
        let position_class = env.find_class(jni_str!("com/lingxia/lxapp/TabBarState$Position"))?;

        let position_enum = match position_int {
            1 => env.get_static_field(
                position_class,
                jni_str!("LEFT"),
                jni_sig!("Lcom/lingxia/lxapp/TabBarState$Position;"),
            )?,
            2 => env.get_static_field(
                position_class,
                jni_str!("RIGHT"),
                jni_sig!("Lcom/lingxia/lxapp/TabBarState$Position;"),
            )?,
            _ => env.get_static_field(
                position_class,
                jni_str!("BOTTOM"),
                jni_sig!("Lcom/lingxia/lxapp/TabBarState$Position;"),
            )?,
        };

        // Create TabBarState object (all parameters non-nullable)
        let obj = env.new_object(
            tab_bar_class,
            jni_sig!("(IIIIILcom/lingxia/lxapp/TabBarState$Position;Ljava/util/List;ZI)V"),
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
        )?;
        Ok(obj)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Create TabBarItem with actual badge and red dot data from Rust
fn create_tab_bar_item<'a>(
    env: &mut Env<'a>,
    item: &lxapp::tabbar::TabBarItem,
) -> Option<JObject<'a>> {
    // Find TabBarItem class
    let tab_item_class = match env.find_class(jni_str!("com/lingxia/lxapp/TabBarItem")) {
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
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/String;Z)V"),
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
    mut env: EnvUnowned,
    _class: JClass,
    applink_url: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let url: String = applink_url.try_to_string(env)?;

        log::info!("[Android] AppLink received: {}", url);
        Ok(0)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Get current active LxApp ID and path from Rust stack
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getCurrentLxApp<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let (current_appid, current_path, current_session_id) = lxapp::get_current_lxapp();

        // Find the CurrentLxApp class (we'll need to create this)
        let current_lxapp_class = env.find_class(jni_str!("com/lingxia/lxapp/CurrentLxApp"))?;

        // Create Java strings
        let appid_str = env.new_string(&current_appid)?;
        let path_str = env.new_string(&current_path)?;

        // Create CurrentLxApp object
        let obj = env.new_object(
            current_lxapp_class,
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;J)V"),
            &[
                (&appid_str).into(),
                (&path_str).into(),
                (current_session_id as jlong).into(),
            ],
        )?;
        Ok(obj)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Get runtime session id for a specific LxApp.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getLxAppSessionId<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let session_id = lxapp::try_get(&appid)
            .map(|lxapp| lxapp.session_id() as jlong)
            .unwrap_or(0);
        Ok(session_id)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Callback from platform (called from Kotlin via NativeAPI)
///
/// # Parameters
/// - `id`: Callback ID for correlating with pending operation
/// - `success`: Whether the operation completed successfully
/// - `data`: When `success=true`, contains JSON payload; when `success=false`, contains error code string (see i18n/err_code)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onCallback(
    mut env: EnvUnowned,
    _class: JClass,
    id: jlong,
    success: jboolean,
    data: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let id = id as u64;
        let success = success;

        let data_str: String = match data.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(e) => {
                error!("[Android] Failed to get data string: {}", e);
                let _ = invoke_callback(id, Err(1000));
                return Ok(false);
            }
        };

        let result = if success {
            Ok(data_str)
        } else {
            Err(data_str.parse::<u32>().unwrap_or(1000))
        };

        if invoke_callback(id, result) {
            Ok(true)
        } else {
            warn!("[Android] Callback not found for id={}", id);
            Ok(false)
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onNativeComponentEvent<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
    component_id: JString<'a>,
    event_name: JString<'a>,
    payload_json: JString<'a>,
    bindings_json: JString<'a>,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let component_id: String = component_id.try_to_string(env)?;
        let event_name: String = event_name.try_to_string(env)?;
        let payload_json: String = payload_json.try_to_string(env)?;
        let bindings_json: String = bindings_json.try_to_string(env)?;

        let accepted = lxapp::on_native_component_event(
            &appid,
            &path,
            &component_id,
            &event_name,
            &payload_json,
            &bindings_json,
        );

        Ok(if accepted {
            true as jboolean
        } else {
            false as jboolean
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Notify native layer that app entered foreground
/// This should be called from LxAppActivity.onStart
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onAppShow(
    mut env: EnvUnowned,
    _class: JClass,
    lxappid: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let lxappid: String = match lxappid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(e) => {
                error!(
                    "[Android] Failed to get lxappid string for onAppShow: {}",
                    e
                );
                return Err(e);
            }
        };

        if let Some(lxapp) = lxapp::try_get(&lxappid) {
            let args = AppServiceEventArgs {
                source: AppServiceEventSource::Host,
                reason: AppServiceEventReason::Foreground,
            }
            .to_json_string();
            let _ = lxapp.appservice_notify(AppServiceEvent::OnShow, Some(args));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Notify native layer that app entered background
/// This should be called from LxAppActivity.onStop
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onAppHide(
    mut env: EnvUnowned,
    _class: JClass,
    lxappid: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let lxappid: String = match lxappid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(e) => {
                error!(
                    "[Android] Failed to get lxappid string for onAppHide: {}",
                    e
                );
                return Err(e);
            }
        };

        if let Some(lxapp) = lxapp::try_get(&lxappid) {
            let args = AppServiceEventArgs {
                source: AppServiceEventSource::Host,
                reason: AppServiceEventReason::Background,
            }
            .to_json_string();
            let _ = lxapp.appservice_notify(AppServiceEvent::OnHide, Some(args));
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Resolve a lx:// URI or sandbox path to a native-consumable URL/path.
///
/// - Accepts `lx://usercache/...`, `lx://userdata/...`, relative paths like `images/1.png`,
///   and absolute paths.
/// - Returns `null` if the path is not accessible inside the app sandbox.
/// - Passes through `http(s)://...` unchanged.
/// - Returns `file://...` for local filesystem paths.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_resolveLxUri<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    input: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let appid: String = match appid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };

        let input: String = match input.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(JString::null());
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return env.new_string(trimmed).or_else(|_| Ok(JString::null()));
        }

        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(JString::null());
        };

        let resolved = if let Some(path) = trimmed.strip_prefix("file://") {
            lxapp.resolve_accessible_path(path).ok()
        } else {
            lxapp.resolve_accessible_path(trimmed).ok()
        };

        let Some(resolved) = resolved else {
            return Ok(JString::null());
        };

        let resolved_str = resolved.to_string_lossy();
        env.new_string(format!("file://{}", resolved_str))
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_openBrowserTab<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    session_id: jlong,
    url: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let appid: String = match appid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        let url: String = match url.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        if session_id <= 0 {
            return Ok(JString::null());
        }

        let tab_id = match lxapp::resolve_owner_lxapp(&appid, session_id as u64) {
            Ok(_owner) => match lxapp::open_internal_browser_tab(&url, None) {
                Ok(tab_id) => tab_id,
                Err(e) => {
                    error!("[Android] openBrowserTab failed: {}", e);
                    return Ok(JString::null());
                }
            },
            Err(e) => {
                error!("[Android] openBrowserTab owner resolve failed: {}", e);
                return Ok(JString::null());
            }
        };

        env.new_string(tab_id).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_browserTabClose(
    mut env: EnvUnowned,
    _class: JClass,
    tab_id: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let tab_id: String = match tab_id.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        Ok(lxapp::close_browser_tab(&tab_id).is_ok())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getBuiltinBrowserAppId<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        env.new_string(lxapp::BUILTIN_BROWSER_APPID)
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_browserTabPathForId<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    tab_id: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let tab_id: String = match tab_id.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        let path = lxapp::browser_tab_path_for_id(&tab_id);
        env.new_string(path).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_handleBrowserAddressInput<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    request_json: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let request_json: String = match request_json.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };

        let Some(response_json) = lxapp::handle_browser_address_input_json(&request_json) else {
            return Ok(JString::null());
        };

        env.new_string(response_json)
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_handleBrowserNavigationPolicy<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    request_json: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let request_json: String = match request_json.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };

        let Some(response_json) = lxapp::handle_browser_navigation_policy_json(&request_json)
        else {
            return Ok(JString::null());
        };

        env.new_string(response_json)
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
