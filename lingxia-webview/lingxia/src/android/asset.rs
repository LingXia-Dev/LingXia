use std::sync::{Arc, Mutex, OnceLock};
use ndk_sys;

// Asset manager wrapper
#[derive(Debug)]
pub(crate) struct AssetManager(pub *mut ndk_sys::AAssetManager);

// Implement Send and Sync for AssetManager
unsafe impl Send for AssetManager {}
unsafe impl Sync for AssetManager {}

// Global asset manager instance
pub static ASSET_MANAGER: OnceLock<Arc<Mutex<AssetManager>>> = OnceLock::new();

impl AssetManager {
    pub fn from_java(ptr: *mut ndk_sys::AAssetManager) -> Self {
        AssetManager(ptr)
    }

    pub fn open(&self, path: &str) -> Option<AssetFile> {
        unsafe {
            let asset = ndk_sys::AAssetManager_open(
                self.0,
                format!("{}\0", path).as_bytes().as_ptr() as *const _,
                ndk_sys::AASSET_MODE_BUFFER as i32,
            );
            if !asset.is_null() {
                Some(AssetFile(asset))
            } else {
                None
            }
        }
    }
}

pub struct AssetFile(*mut ndk_sys::AAsset);

impl AssetFile {
    pub fn read_all(&self) -> Option<Vec<u8>> {
        unsafe {
            let length = ndk_sys::AAsset_getLength64(self.0) as usize;
            let mut buffer = vec![0u8; length];
            let bytes_read = ndk_sys::AAsset_read(self.0, buffer.as_mut_ptr() as *mut _, length);
            if bytes_read > 0 {
                Some(buffer)
            } else {
                None
            }
        }
    }

    pub fn get_mime_type(&self, path: &str) -> &'static str {
        if path.ends_with(".html") {
            "text/html"
        } else if path.ends_with(".js") {
            "application/javascript"
        } else if path.ends_with(".css") {
            "text/css"
        } else {
            "application/octet-stream"
        }
    }
}

impl Drop for AssetFile {
    fn drop(&mut self) {
        unsafe {
            ndk_sys::AAsset_close(self.0);
        }
    }
} 