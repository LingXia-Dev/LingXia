use jni::sys::{JNIEnv, jobject};
use miniapp::AssetReader;
use ndk_sys;
use std::sync::{Arc, Mutex, OnceLock};

// Asset manager wrapper
#[derive(Clone)]
pub(crate) struct AssetManager(pub *mut ndk_sys::AAssetManager);

// Implement Send and Sync for AssetManager
unsafe impl Send for AssetManager {}
unsafe impl Sync for AssetManager {}

// Global asset manager instance
pub static ASSET_MANAGER: OnceLock<Arc<Mutex<AssetManager>>> = OnceLock::new();

impl AssetReader for AssetManager {
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        unsafe {
            let asset = ndk_sys::AAssetManager_open(
                self.0,
                path.as_ptr() as *const _,
                ndk_sys::AASSET_MODE_BUFFER as i32,
            );

            if asset.is_null() {
                return Err("Failed to open asset".into());
            }

            let length = ndk_sys::AAsset_getLength64(asset) as usize;
            let mut buffer = vec![0u8; length];
            let read = ndk_sys::AAsset_read(asset, buffer.as_mut_ptr() as *mut _, length) as i32;

            ndk_sys::AAsset_close(asset);

            if read == length as i32 {
                Ok(buffer)
            } else {
                Err("Failed to read asset completely".into())
            }
        }
    }
}

impl AssetManager {
    pub fn from_java(env: *mut JNIEnv, asset_manager: jobject) -> Result<Self, &'static str> {
        unsafe {
            let ptr = ndk_sys::AAssetManager_fromJava(env, asset_manager);
            if ptr.is_null() {
                Err("Failed to get AAssetManager from Java object")
            } else {
                Ok(AssetManager(ptr))
            }
        }
    }
}
