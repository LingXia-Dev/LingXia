use jni::objects::{JClass, JObject, JString};
use jni::strings::JNIString;
use jni::sys::{jboolean, jdouble, jint, jlong};
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
    CloseReason, CreatePageInstanceRequest, LxAppDelegate, LxAppUiEventType, OrientationConfig,
    PageInstanceEvent, PageOrientation, PageOwner, PageTarget, PresentationKind, SceneId,
};
use std::sync::OnceLock;

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

fn initialize_jni(vm: JavaVM) {
    let _ = JAVA_VM.set(vm);
}

/// Run closure with a JNI `Env`, attaching current thread when needed.
///
/// This is the public helper for app-side native routes that need to call Java/Kotlin
/// from non-JNI threads.
pub fn with_env<T, E>(f: impl FnOnce(&mut Env) -> Result<T, E>) -> Result<T, E>
where
    E: From<jni::errors::Error>,
{
    let vm = JAVA_VM
        .get()
        .ok_or_else(|| E::from(jni::errors::Error::UninitializedJavaVM))?;
    vm.attach_current_thread(f)
}

fn normalize_lookup_path(path: &str) -> &str {
    let path = path.split('?').next().unwrap_or(path);
    path.split('#').next().unwrap_or(path)
}

fn resolve_page_instance_id(appid: &str, path: &str, session_id: u64) -> Option<String> {
    let lxapp_instance = lxapp::try_get(appid)?;
    if lxapp_instance.session_id() != session_id {
        return None;
    }

    let resolved_path = lxapp_instance
        .find_page_path(normalize_lookup_path(path))
        .unwrap_or_else(|| normalize_lookup_path(path).to_string());
    let id = lxapp_instance.page_instance_id_for_path(&resolved_path)?;
    if !id.is_empty() {
        let _ = lxapp::touch_page_instance_by_id(&id);
    }
    Some(id)
}

fn parse_close_reason(reason: &str) -> CloseReason {
    match reason.trim().to_ascii_lowercase().as_str() {
        "user" => CloseReason::User,
        "owner_closed" => CloseReason::OwnerClosed,
        "app_closed" | "appclose" | "close" => CloseReason::AppClosed,
        "programmatic" => CloseReason::Programmatic,
        "failed" | "presentation_failed" => CloseReason::Unknown,
        _ => CloseReason::Unknown,
    }
}

fn notify_page_instance_event(
    env: &mut EnvUnowned,
    page_instance_id: JString,
    event: PageInstanceEvent,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let page_instance_id: String = page_instance_id.try_to_string(env)?;
        Ok(lxapp::notify_page_instance_by_id(&page_instance_id, event).is_ok() as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
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
            env.exception_clear();
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
        CachedClass::Lingxia,
        CachedClass::LxApp,
        CachedClass::PreviewMediaPayload,
        CachedClass::LxAppMedia,
        CachedClass::LxAppDevice,
        CachedClass::LxAppLocation,
        CachedClass::LxAppSurface,
        CachedClass::LxAppToast,
        CachedClass::LxAppModal,
        CachedClass::LxAppActionSheet,
        CachedClass::LxAppPicker,
        CachedClass::LxAppFile,
        CachedClass::ComponentRouter,
        CachedClass::LxAppPullToRefresh,
        CachedClass::UpdateManager,
        CachedClass::LxAppCapsule,
        CachedClass::LxAppWifi,
        CachedClass::LxAppNetwork,
        CachedClass::AppScreenshot,
        CachedClass::LxAppShare,
    ];

    for class in classes {
        init_cached_java_class(env, class);
    }
}

#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::os::raw::c_void) -> jint {
    crate::logging::init();
    initialize_jni(vm.clone());
    lingxia_platform::initialize_jni(vm.clone());
    lingxia_webview::platform::android::initialize_jni(vm);

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_lingxiaInit<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    data_dir: JString<'a>,
    cache_dir: JString<'a>,
    asset_manager: JObject<'a>,
    application_context: JObject<'a>,
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
            "Initializing Lingxia SDK with data_dir: {}, cache_dir: {}, locale: {}",
            data_dir_str,
            cache_dir_str,
            locale_str
        );

        let platform = unsafe {
            lingxia_platform::Platform::from_java(
                env,
                asset_manager.as_raw(),
                application_context.as_raw(),
                data_dir_str,
                cache_dir_str,
                locale_str,
            )
        }
        .map_err(|_| jni::errors::Error::JniCall(jni::errors::JniError::Unknown))?;

        let home_app_id = match crate::init_with_platform(platform) {
            Ok(home_app_id) => home_app_id,
            Err(error) => {
                error!("Failed to initialize LingXia runtime: {error}");
                return Ok(JString::null());
            }
        };

        // Return the home appid
        match home_app_id.into_lxapp_id() {
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_getDisplayLanguage<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    env.with_env(|env| env.new_string(crate::app::display_language()))
        .resolve::<LogErrorAndDefault>()
}

/// The launch face is on screen, in this appearance — the one the system
/// splash resolved. Runs before runtime initialization; the launch face must
/// never wait on the runtime.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_splashMarkLaunchFace<'a>(
    _env: EnvUnowned<'a>,
    _class: JClass<'a>,
    dark: jboolean,
) {
    crate::splash::mark_launch_face(dark)
}

/// The host-declared page floor for one appearance, as `#RRGGBB`.
///
/// Empty means the host declared none and the platform should keep its own
/// system background. Native chrome paints with this wherever it borders the
/// page — the strip a pull-to-refresh opens above it, the container a
/// navigation transition slides views across — because a WebView cannot be
/// asked for its document colour in time to paint the frame already on
/// screen.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_pageBackgroundColor<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    dark: jboolean,
) -> JString<'a> {
    env.with_env(|env| {
        env.new_string(lingxia_app_context::page_background_color(dark).unwrap_or_default())
    })
    .resolve::<LogErrorAndDefault>()
}

/// The configured minimum hold, in milliseconds.
///
/// Read at dismissal rather than at attach: this resolves from `app.json`,
/// which the runtime loads well after the bootstrap activity has already put
/// the cover on screen.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_splashMinDurationMs(
    _env: EnvUnowned,
    _class: JClass,
) -> jlong {
    jlong::try_from(lingxia_app_context::splash_min_duration().as_millis()).unwrap_or(jlong::MAX)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_forwardHostLog(
    mut env: EnvUnowned,
    _class: JClass,
    level: jint,
    category: JString,
    appid: JString,
    path: JString,
    message: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let category: String = category.try_to_string(env)?;
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let message: String = message.try_to_string(env)?;
        Ok(crate::logging::forward_host_log(
            level, &category, &appid, &path, &message,
        ))
    })
    .resolve::<LogErrorAndDefault>()
}

/// Set the runtime log threshold (0=verbose … 4=error). See
/// [`crate::logging::set_log_level`].
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_setLogLevel(
    _env: EnvUnowned,
    _class: JClass,
    level: jint,
) {
    crate::logging::set_log_level(level);
}

/// Whether a host log at `level` would be recorded. See
/// [`crate::logging::host_log_enabled`].
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_hostLogEnabled(
    _env: EnvUnowned,
    _class: JClass,
    level: jint,
) -> jboolean {
    crate::logging::host_log_enabled(level) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_onPageShow(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_findWebView<'a>(
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

        let Some(page_instance_id) = resolve_page_instance_id(&appid, &path, session_id as u64)
        else {
            // Browser tabs showing external documents have no bound lxapp
            // PageInstance — the WebView belongs to the tab, not a page.
            // Resolve it directly from the webview registry by webtag.
            let webtag = lingxia_webview::WebTag::new(&appid, &path, Some(session_id as u64));
            if let Some(webview) = lingxia_webview::runtime::find_webview(&webtag) {
                return match env.new_local_ref(webview.get_java_webview()) {
                    Ok(local_ref) => Ok(unsafe { JObject::from_raw(env, local_ref.into_raw()) }),
                    Err(e) => {
                        error!("Failed to create local reference to WebView: {:?}", e);
                        Ok(JObject::null())
                    }
                };
            }
            let (current_appid, current_path, current_session_id) = lxapp::get_current_lxapp();
            if current_appid == appid
                && current_session_id == session_id as u64
                && normalize_lookup_path(&current_path) == normalize_lookup_path(&path)
            {
                error!(
                    "WebView resolve failed for current page {}:{} (session={})",
                    appid, path, session_id
                );
            } else {
                info!(
                    "Ignoring stale WebView lookup for {}:{} (session={})",
                    appid, path, session_id
                );
            }
            return Ok(JObject::null());
        };
        let Some(page) = lxapp::find_page_by_instance_id(&page_instance_id) else {
            error!(
                "Page instance not found for {}:{} (session={}, page_instance_id={})",
                appid, path, session_id, page_instance_id
            );
            return Ok(JObject::null());
        };
        let Some(webview) = page.webview() else {
            return Ok(JObject::null());
        };

        // A cached page re-enters the transition immediately; stamp the live
        // scheme before it becomes visible, not after the show animation.
        if let Some(app) = lxapp::try_get(&page.appid()) {
            app.republish_page_scheme(&page);
        }

        match env.new_local_ref(webview.get_java_webview()) {
            Ok(local_ref) => Ok(unsafe { JObject::from_raw(env, local_ref.into_raw()) }),
            Err(e) => {
                error!("Failed to create local reference to WebView: {:?}", e);
                Ok(JObject::null())
            }
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_findWebViewByPageInstanceId<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    page_instance_id: JString<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let page_instance_id: String = page_instance_id.try_to_string(env)?;
        let page_instance_id = page_instance_id.trim();
        if page_instance_id.is_empty() {
            return Ok(JObject::null());
        }
        let _ = lxapp::touch_page_instance_by_id(page_instance_id);
        let Some(page) = lxapp::find_page_by_instance_id(page_instance_id) else {
            return Ok(JObject::null());
        };
        let Some(webview) = page.webview() else {
            return Ok(JObject::null());
        };

        // A cached page re-enters the transition immediately; stamp the live
        // scheme before it becomes visible, not after the show animation.
        if let Some(app) = lxapp::try_get(&page.appid()) {
            app.republish_page_scheme(&page);
        }

        match env.new_local_ref(webview.get_java_webview()) {
            Ok(local_ref) => Ok(unsafe { JObject::from_raw(env, local_ref.into_raw()) }),
            Err(e) => {
                error!("Failed to create local reference to WebView: {:?}", e);
                Ok(JObject::null())
            }
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_notifyPageInstanceMounted(
    mut env: EnvUnowned,
    _class: JClass,
    page_instance_id: JString,
) -> jboolean {
    notify_page_instance_event(&mut env, page_instance_id, PageInstanceEvent::Mounted)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_onHostAppearanceChanged(
    _env: EnvUnowned,
    _class: JClass,
) {
    lxapp::refresh_auto_appearances();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_notifyPageInstanceVisible(
    mut env: EnvUnowned,
    _class: JClass,
    page_instance_id: JString,
) -> jboolean {
    notify_page_instance_event(&mut env, page_instance_id, PageInstanceEvent::Visible)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_notifyPageInstanceHidden(
    mut env: EnvUnowned,
    _class: JClass,
    page_instance_id: JString,
    reason: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let page_instance_id: String = page_instance_id.try_to_string(env)?;
        let reason: String = reason.try_to_string(env)?;
        Ok(lxapp::notify_page_instance_by_id(
            &page_instance_id,
            PageInstanceEvent::Hidden {
                reason: parse_close_reason(&reason),
            },
        )
        .is_ok() as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_disposePageInstance(
    mut env: EnvUnowned,
    _class: JClass,
    page_instance_id: JString,
    reason: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let page_instance_id: String = page_instance_id.try_to_string(env)?;
        let reason: String = reason.try_to_string(env)?;
        Ok(
            lxapp::dispose_page_instance_by_id(&page_instance_id, parse_close_reason(&reason))
                .is_ok() as jboolean,
        )
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_onSurfaceClosed(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    id: JString,
    reason: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let id: String = id.try_to_string(env)?;
        let reason: String = reason.try_to_string(env)?;
        if let Some(lxapp) = lxapp::try_get(&appid) {
            let _ = lxapp.forget_surface(&id);
        }
        #[cfg(feature = "standard")]
        {
            Ok(lingxia_logic::notify_surface_closed(&id, &reason) as jboolean)
        }
        #[cfg(not(feature = "standard"))]
        {
            Ok(false as jboolean)
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// Function for LxAppActivity class to handle the LxApp close event
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_onLxAppClosed(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_getNavigationBarState<'a>(
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
        let nav_state = lxapp.get_navbar_state(&path);

        // Find the NavigationBarState class
        let nav_bar_class =
            env.find_class(jni_str!("com/lingxia/lxapp/chrome/NavigationBarState"))?;

        let style = lxapp.resolved_navigation_bar_style(&path);
        let capsule = lxapp.resolved_capsule_style();
        let bg_color_int = style.background_color.argb() as i32;
        let foreground_color_int = style.foreground_color.argb() as i32;
        let divider_color_int = style.divider_color.argb() as i32;
        let text_style_value = style.foreground_text_style();

        // Create Java strings
        let title_text = env.new_string(nav_state.title())?;
        let text_style = env.new_string(text_style_value)?;

        let obj = env.new_object(
            nav_bar_class,
            jni_sig!("(IIILjava/lang/String;Ljava/lang/String;ZZZIIII)V"),
            &[
                (bg_color_int as jint).into(),
                (foreground_color_int as jint).into(),
                (divider_color_int as jint).into(),
                (&text_style).into(),
                (&title_text).into(),
                (nav_state.show_navbar as jboolean).into(),
                (nav_state.show_back_button as jboolean).into(),
                (nav_state.home_button_visible() as jboolean).into(),
                (capsule.background_color.argb() as jint).into(),
                (capsule.foreground_color.argb() as jint).into(),
                (capsule.divider_color.argb() as jint).into(),
                (capsule.interaction_color.argb() as jint).into(),
            ],
        )?;
        Ok(obj)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Check if pull-to-refresh is enabled for a specific page
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_isPullDownRefreshEnabled<'a>(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_getPageOrientation<'a>(
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
pub extern "C" fn Java_com_lingxia_app_NativeApi_onLxappEvent(
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
pub extern "C" fn Java_com_lingxia_app_NativeApi_onKeyEvent(
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
            KEY_EVENT_DOWN => lxapp::lifecycle::key_events::has_key_down(&appid, session_id),
            KEY_EVENT_UP => lxapp::lifecycle::key_events::has_key_up(&appid, session_id),
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
pub extern "C" fn Java_com_lingxia_app_NativeApi_onDeviceOrientationChanged(
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

// Function to notify the Rust layer that an LxApp has been opened
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_onLxAppOpened<'a>(
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
        let Some(lxapp_instance) = lxapp::try_get(&appid) else {
            return env.new_string("");
        };
        if lxapp_instance.session_id() != session_id as u64 {
            return env.new_string("");
        }

        let resolved_path = lxapp::create_page_instance(CreatePageInstanceRequest {
            owner: PageOwner::Scene(SceneId("system".to_string())),
            appid: appid.clone(),
            target: PageTarget::Path(path),
            query: None,
            surface: PresentationKind::Window,
        })
        .map(|created| created.resolved_path)
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_getLxAppInfo<'a>(
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
        let cache_dir_str = env.new_string(lxapp.user_cache_dir.to_string_lossy())?;

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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_getLxAppMoreActions<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let json = lxapp::try_get(&appid)
            .map(|app| app.more_actions_json())
            .unwrap_or_else(|| r#"{"generation":0,"items":[]}"#.to_string());
        env.new_string(json)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// Get TabBar configuration using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_getTabBarState<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;

        let Some(lxapp) = lxapp::try_get(&appid) else {
            return Ok(JObject::null());
        };
        let Some(tab_bar_config) = lxapp.get_tabbar() else {
            return Ok(JObject::null());
        };
        let Some(resolved_style) = lxapp.resolved_tabbar_style() else {
            return Ok(JObject::null());
        };

        // Find the TabBarState class
        let tab_bar_class = env.find_class(jni_str!("com/lingxia/lxapp/chrome/TabBarState"))?;

        let background_color = resolved_style
            .background_color
            .map(|color| color.argb() as i32)
            .unwrap_or(0);

        let selected_color = resolved_style.selected_foreground_color.argb() as i32;

        let color = resolved_style.foreground_color.argb() as i32;

        let border_style = resolved_style
            .divider_color
            .map(|color| color.argb() as i32)
            .unwrap_or(0);

        let dimension = 64;

        // Create TabBarItem list
        let array_list_class = env.find_class(jni_str!("java/util/ArrayList"))?;

        let tab_items_list = env.new_object(array_list_class, jni_sig!("()V"), &[])?;

        for (index, item) in tab_bar_config.visible_items() {
            if let Some(tab_item) = create_tab_bar_item(
                env,
                index as i32,
                item,
                tab_bar_config.selected_index == index as i32,
            ) {
                let _ = env.call_method(
                    &tab_items_list,
                    jni_str!("add"),
                    jni_sig!("(Ljava/lang/Object;)Z"),
                    &[(&tab_item).into()],
                );
            } else {
                log::warn!(
                    "[Android] Failed to create TabBar item in getTabBarState for {}",
                    &item.page_path
                );
            }
        }

        // Create Position enum
        let position_class =
            env.find_class(jni_str!("com/lingxia/lxapp/chrome/TabBarState$Position"))?;

        let position_enum = env.get_static_field(
            position_class,
            jni_str!("BOTTOM"),
            jni_sig!("Lcom/lingxia/lxapp/chrome/TabBarState$Position;"),
        )?;

        // Create TabBarState object (all parameters non-nullable)
        let obj = env.new_object(
            tab_bar_class,
            jni_sig!("(IIIIILcom/lingxia/lxapp/chrome/TabBarState$Position;Ljava/util/List;ZII)V"),
            &[
                background_color.into(),
                selected_color.into(),
                color.into(),
                border_style.into(),
                dimension.into(),
                (&position_enum).into(),
                (&tab_items_list).into(),
                tab_bar_config.is_effectively_visible().into(),
                tab_bar_config.selected_index.into(),
                tab_bar_config.compact_overflow_slot_index().into(),
            ],
        )?;
        Ok(obj)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Create TabBarItem with actual badge and red dot data from Rust
fn create_tab_bar_item<'a>(
    env: &mut Env<'a>,
    index: i32,
    item: &lxapp::tabbar::TabBarItem,
    selected: bool,
) -> Option<JObject<'a>> {
    // Find TabBarItem class
    let tab_item_class = match env.find_class(jni_str!("com/lingxia/lxapp/chrome/TabBarItem")) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Create strings
    let page_path = match env.new_string(&item.page_path) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let text = match env.new_string(item.text.as_deref().unwrap_or("")) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let icon_path = match env.new_string(item.icon_path.as_deref().unwrap_or("")) {
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
    env.new_object(
        tab_item_class,
        jni_sig!("(ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;ZLjava/lang/String;Z)V"),
        &[
            index.into(),
            (&page_path).into(),
            (&text).into(),
            (&icon_path).into(),
            selected.into(),
            (&badge_jstring).into(),
            item.has_red_dot.into(), // Use actual red dot data from Rust
        ],
    )
    .ok()
}

/// Handle DeepLink URL by processing the path without host
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_onAppLinkReceived(
    mut env: EnvUnowned,
    _class: JClass,
    applink_url: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let url: String = applink_url.try_to_string(env)?;
        Ok(lingxia_service::applink::handle(&url) as jint)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Get current active LxApp ID and path from Rust stack
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_getCurrentLxApp<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject, jni::errors::Error> {
        let (current_appid, current_path, current_session_id) = lxapp::get_current_lxapp();

        let current_lxapp_class = env.find_class(jni_str!("com/lingxia/app/CurrentLxApp"))?;

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
pub extern "system" fn Java_com_lingxia_app_NativeApi_getLxAppSessionId<'a>(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_onCallback(
    mut env: EnvUnowned,
    _class: JClass,
    id: jlong,
    success: jboolean,
    data: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let id = id as u64;

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
pub extern "system" fn Java_com_lingxia_app_NativeApi_onNativeComponentEvent<'a>(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_onAppShow(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_onAppHide(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_resolveLxUri<'a>(
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
pub extern "system" fn Java_com_lingxia_app_NativeApi_openBrowserTab<'a>(
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

        let tab_id = match crate::browser::open_for_app(&appid, session_id as u64, &url, None) {
            Ok(tab_id) => tab_id,
            Err(e) => {
                error!("[Android] openBrowserTab failed: {}", e);
                return Ok(JString::null());
            }
        };

        env.new_string(tab_id).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_openStandaloneBrowserTab<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    session_id: jlong,
    url: JString<'a>,
    ephemeral_web_data: jboolean,
    url_callback: jboolean,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let appid = match appid.try_to_string(env) {
            Ok(value) => value.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        let url = match url.try_to_string(env) {
            Ok(value) => value.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        if session_id <= 0 {
            return Ok(JString::null());
        }
        let data_mode = if ephemeral_web_data {
            lingxia_webview::WebViewDataMode::Ephemeral
        } else {
            lingxia_webview::WebViewDataMode::ProfileDefault
        };
        let tab_id = match crate::browser::open_standalone_for_app(
            &appid,
            session_id as u64,
            &url,
            None,
            data_mode,
            url_callback,
        ) {
            Ok(tab_id) => tab_id,
            Err(error) => {
                error!("[Android] openStandaloneBrowserTab failed: {}", error);
                return Ok(JString::null());
            }
        };
        env.new_string(tab_id).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Open an aside tab in the shared in-app browser: self chrome minus the
/// address bar (compact `{ url, as: 'aside' }`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_openAsideBrowserTab<'a>(
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

        let tab_id = match crate::browser::open_aside_for_app(&appid, session_id as u64, &url, None)
        {
            Ok(tab_id) => tab_id,
            Err(e) => {
                error!("[Android] openAsideBrowserTab failed: {}", e);
                return Ok(JString::null());
            }
        };

        env.new_string(tab_id).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Whether the tab belongs to the API-managed aside browser group.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_browserTabIsAside(
    mut env: EnvUnowned,
    _class: JClass,
    tab_id: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let tab_id: String = match tab_id.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        Ok(crate::browser::tab_is_aside(&tab_id))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_browserTabClose(
    mut env: EnvUnowned,
    _class: JClass,
    tab_id: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let tab_id: String = match tab_id.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        Ok(crate::browser::close(&tab_id).is_ok())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_browserTabNavigate(
    mut env: EnvUnowned,
    _class: JClass,
    tab_id: JString,
    url: JString,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let tab_id: String = match tab_id.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        let url: String = match url.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        Ok(crate::browser::navigate(&tab_id, &url).is_ok())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_browserTabActivate(
    mut env: EnvUnowned,
    _class: JClass,
    tab_id: JString,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let tab_id: String = tab_id.try_to_string(env)?.to_string();
        crate::browser::mark_active(&tab_id);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_getBuiltinBrowserAppId<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        env.new_string(crate::browser::APP_ID)
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_browserTabPathForId<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    tab_id: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let tab_id: String = match tab_id.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        let path = crate::browser::tab_path(&tab_id);
        env.new_string(path).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_setSurfaceWidth(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    width: jdouble,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = match appid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        Ok(lxapp::try_get(&appid)
            .map(|lxapp| lxapp.set_surface_width(width))
            .unwrap_or(false) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_setSurfaceViewport(
    mut env: EnvUnowned,
    _class: JClass,
    appid: JString,
    width: jdouble,
    height: jdouble,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let appid: String = match appid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(false),
        };
        Ok(lxapp::try_get(&appid)
            .map(|lxapp| lxapp.set_surface_viewport(width, height))
            .unwrap_or(false) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_surfaceDerivedLayout<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let appid: String = match appid.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };
        let json = lxapp::try_get(&appid)
            .and_then(|lxapp| lxapp.surface_derived_layout())
            .and_then(|layout| serde_json::to_string(&layout).ok())
            .unwrap_or_else(|| "null".to_string());
        env.new_string(json).or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_urlCallbackDispatch<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    url: JString<'a>,
) -> jboolean {
    env.with_env(|env| -> Result<jboolean, jni::errors::Error> {
        let url: String = url.try_to_string(env)?;
        Ok(lingxia_webview::url_callback::dispatch(&url) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_webviewLoadErrorDocument<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    url: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let url: String = url.try_to_string(env)?;
        env.new_string(crate::webview_error::load_error_document(&url))
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_handleBrowserNavigationPolicy<'a>(
    mut env: EnvUnowned<'a>,
    _class: JClass<'a>,
    request_json: JString<'a>,
) -> JString<'a> {
    env.with_env(|env| -> Result<JString, jni::errors::Error> {
        let request_json: String = match request_json.try_to_string(env) {
            Ok(s) => s.to_string(),
            Err(_) => return Ok(JString::null()),
        };

        let Some(response_json) = crate::browser::classify_navigation_json(&request_json) else {
            return Ok(JString::null());
        };

        env.new_string(response_json)
            .or_else(|_| Ok(JString::null()))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_getAppCapabilities(
    _env: EnvUnowned,
    _class: JClass,
) -> jint {
    crate::capabilities::app_capabilities() as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_app_NativeApi_shouldEnableWebViewDebugging(
    _env: EnvUnowned,
    _class: JClass,
) -> jboolean {
    crate::should_enable_webview_debugging()
}
