#![allow(non_snake_case)]

use std::sync::OnceLock;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use jni::objects::{JClass, JObject, JString};
use jni::sys::jint;
use jni::JNIEnv;
use log::{error, info};
use android_logger::Config;

mod webview;
use webview::WebViewManager;

pub static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();

#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: jni::JavaVM, _: *mut std::os::raw::c_void) -> jint {
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("RustNative"),
    );

    // Store JavaVM globally
    let _ = JAVA_VM.set(vm);

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnWebViewCreated(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
    java_webview: JObject,
) -> jint {
    let app_id: String = match env.get_string(&app_id) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get app_id string: {:?}", e);
            return -1;
        }
    };

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get path string: {:?}", e);
            return -1;
        }
    };

    match WebViewManager::on_webview_registered(&mut env, app_id, path, java_webview) {
        Ok(_) => 0,
        Err(e) => {
            error!("Failed to create WebView: {:?}", e);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnMiniAppDestroy(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    match WebViewManager::destroy_all_webviews() {
        Ok(_) => 0,
        Err(e) => {
            log::error!("Failed to destroy WebViews: {:?}", e);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandlePostMessage(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
    message: JString,
) -> jint {
    let app_id: String = env
        .get_string(&app_id)
        .expect("Couldn't get app_id string")
        .into();
    let path: String = env
        .get_string(&path)
        .expect("Couldn't get path string")
        .into();
    let message: String = env
        .get_string(&message)
        .expect("Couldn't get message string")
        .into();

    match WebViewManager::handle_post_message(&mut env, app_id, path, message) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("Failed to handle post message: {:?}", e);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageStarted(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    match WebViewManager::on_page_started(&mut env, app_id, path) {
        Ok(_) => 0,
        Err(e) => {
            error!("Error in on_page_started: {}", e);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageFinished(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    match WebViewManager::on_page_finished(&mut env, app_id, path) {
        Ok(_) => 0,
        Err(e) => {
            error!("Error in on_page_finished: {}", e);
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeShouldOverrideUrlLoading(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    url: JString,
) -> jint {
    let app_id: String = env
        .get_string(&app_id)
        .expect("Couldn't get app_id string")
        .into();
    let url: String = env
        .get_string(&url)
        .expect("Couldn't get url string")
        .into();

    match WebViewManager::should_override_url_loading(&mut env, app_id, url) {
        Ok(should_override) => {
            if should_override {
                1
            } else {
                0
            }
        }
        Err(e) => {
            log::error!("Failed to handle url override: {:?}", e);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeGetExistingWebView<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    app_id: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let app_id: String = match env.get_string(&app_id) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get app_id string: {:?}", e);
            return JObject::null();
        }
    };

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get path string: {:?}", e);
            return JObject::null();
        }
    };

    match WebViewManager::get_existing_webview(&mut env, &app_id, &path) {
        Ok(Some(webview)) => webview,
        Ok(None) => JObject::null(),
        Err(e) => {
            error!("Failed to get existing WebView: {:?}", e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnMiniAppHidden(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) -> jint {
    let app_id: String = match env.get_string(&app_id) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get app_id string: {:?}", e);
            return -1;
        }
    };

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get path string: {:?}", e);
            return -1;
        }
    };

    info!("Mini app hidden: app_id={}, path={}", app_id, path);
    0
}

#[no_mangle]
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeOnMiniAppInited(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    cache_dir: JString,
    asset_manager: JObject,
) -> jint {
    // Get data and cache directories
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get data dir string: {:?}", e);
            return -1;
        }
    };

    let cache_dir: String = match env.get_string(&cache_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get cache dir string: {:?}", e);
            return -1;
        }
    };

    info!("MiniApp initialized with:");
    info!("  Data dir: {}", data_dir);
    info!("  Cache dir: {}", cache_dir);

    // Demo: Create and write to a file in data directory
    let demo_file_path = PathBuf::from(&data_dir).join("demo.txt");
    if let Err(e) = fs::write(&demo_file_path, "Hello from Rust native layer!") {
        error!("Failed to write demo file: {:?}", e);
    } else {
        info!("Successfully wrote to demo file: {:?}", demo_file_path);
    }

    // Demo: Read the file back
    match fs::read_to_string(&demo_file_path) {
        Ok(contents) => {
            info!("Read from demo file: {}", contents);
        }
        Err(e) => {
            error!("Failed to read demo file: {:?}", e);
        }
    }

    // Demo: Read asset using NDK AAssetManager
    let asset_manager = unsafe {
        ndk_sys::AAssetManager_fromJava(env.get_raw() as *mut _, asset_manager.as_raw() as *mut _)
    };

    unsafe {
        let asset = ndk_sys::AAssetManager_open(
            asset_manager,
            b"readme.txt\0".as_ptr() as *const u8,
            ndk_sys::AASSET_MODE_BUFFER as i32,
        );
        
        if !asset.is_null() {
            let length = ndk_sys::AAsset_getLength64(asset) as usize;
            let mut buffer = vec![0u8; length];
            let bytes_read = ndk_sys::AAsset_read(asset, buffer.as_mut_ptr() as *mut _, length);
            
            if bytes_read > 0 {
                if let Ok(content) = String::from_utf8(buffer) {
                    info!("Read from assets/readme.txt using NDK: {}", content);
                }
            } else {
                error!("Failed to read asset content");
            }
            
            ndk_sys::AAsset_close(asset);
        } else {
            error!("Failed to open asset file");
        }
    }

    0
}
