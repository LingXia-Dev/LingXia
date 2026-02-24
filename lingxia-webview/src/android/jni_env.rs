use jni::objects::JClass;
use jni::refs::Global;
use jni::{Env, JavaVM, jni_str};
use log::error;
use std::sync::{Arc, OnceLock};

// Static variables for JNI environment access
pub static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();
static LINGXIA_WEBVIEW_CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();

/// Initialize JNI environment - should be called once from JNI_OnLoad
pub fn initialize_jni(vm: JavaVM) {
    // Cache LingXiaWebView class reference before moving vm
    let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        match env.find_class(jni_str!("com/lingxia/webview/LingXiaWebView")) {
            Ok(class) => match env.new_global_ref(class) {
                Ok(global_ref) => {
                    let _ = LINGXIA_WEBVIEW_CLASS.set(global_ref);
                }
                Err(e) => {
                    error!("Failed to create global reference for LingXiaWebView class: {e}");
                }
            },
            Err(e) => {
                error!("Failed to find LingXiaWebView class during initialization: {e}");
            }
        }
        Ok(())
    });

    let _ = JAVA_VM.set(Arc::new(vm));
}

/// Run a closure with a JNI `Env` reference, attaching the current thread if needed.
pub fn with_env<T, E>(f: impl FnOnce(&mut Env) -> Result<T, E>) -> Result<T, E>
where
    E: From<jni::errors::Error>,
{
    let vm = JAVA_VM
        .get()
        .ok_or(jni::errors::Error::UninitializedJavaVM)?;
    vm.attach_current_thread(f)
}

/// Get cached LingXiaWebView class reference
pub(crate) fn get_lingxia_webview_class() -> Option<&'static Global<JClass<'static>>> {
    LINGXIA_WEBVIEW_CLASS.get()
}
