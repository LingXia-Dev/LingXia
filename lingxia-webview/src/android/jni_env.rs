use jni::objects::GlobalRef;
use jni::{JNIEnv, JavaVM};
use log::error;
use std::sync::{Arc, OnceLock};

// Static variables for JNI environment access
pub static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();
static MAIN_THREAD_ID: OnceLock<std::thread::ThreadId> = OnceLock::new();
static LINGXIA_WEBVIEW_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Initialize JNI environment - should be called once from JNI_OnLoad
pub fn initialize_jni(vm: JavaVM) {
    // Cache LingXiaWebView class reference before moving vm
    if let Ok(mut env) = vm.get_env() {
        if let Ok(class) = env.find_class("com/lingxia/webview/LingXiaWebView") {
            if let Ok(global_ref) = env.new_global_ref(class) {
                let _ = LINGXIA_WEBVIEW_CLASS.set(global_ref);
            } else {
                log::error!("Failed to create global reference for LingXiaWebView class");
            }
        } else {
            log::error!("Failed to find LingXiaWebView class during initialization");
        }
    }

    let _ = JAVA_VM.set(Arc::new(vm));
    let _ = MAIN_THREAD_ID.set(std::thread::current().id());
}

/// Get JNIEnv for current thread
pub fn get_env() -> Result<JNIEnv<'static>, Box<dyn std::error::Error>> {
    let vm = JAVA_VM.get().ok_or("JavaVM not initialized")?;

    // Check if we're on the main thread
    let current_thread = std::thread::current().id();
    let is_main_thread = MAIN_THREAD_ID
        .get()
        .map(|main_id| *main_id == current_thread)
        .unwrap_or(false);

    if is_main_thread {
        // If we're on the main thread, get the env
        match vm.get_env() {
            Ok(env) => unsafe {
                JNIEnv::from_raw(env.get_raw()).map_err(|e| {
                    error!("JNI error: {:?}", e);
                    e.into()
                })
            },
            Err(e) => {
                error!("Failed to get JNI env for main thread: {:?}", e);
                Err(e.into())
            }
        }
    } else {
        // If we're not on the main thread, attach as daemon to avoid lifecycle issues
        match vm.attach_current_thread_as_daemon() {
            Ok(env) => Ok(env),
            Err(e) => {
                error!("Failed to attach thread as daemon: {:?}", e);
                Err(e.into())
            }
        }
    }
}
/// Get cached LingXiaWebView class reference
pub fn get_lingxia_webview_class() -> Option<&'static GlobalRef> {
    LINGXIA_WEBVIEW_CLASS.get()
}

