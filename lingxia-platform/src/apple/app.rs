use super::ffi;
use crate::error::PlatformError;
use crate::{AppRuntime, AssetFileEntry};
use std::io::{Cursor, Read};
use std::path::PathBuf;

/// Platform implementation for Apple platforms (iOS/macOS)
#[derive(Clone)]
pub struct Platform {
    pub data_dir: String,
    pub cache_dir: String,
    pub locale: String,
}

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

impl Platform {
    /// Create a new Platform instance
    pub fn new(data_dir: String, cache_dir: String, locale: String) -> Result<Self, PlatformError> {
        Ok(Platform {
            data_dir,
            cache_dir,
            locale,
        })
    }
}

impl AppRuntime for Platform {
    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    /// Copy media URI to a local file path
    fn copy_media_uri_to_path(
        &self,
        uri: &str,
        dest_path: &std::path::Path,
        _kind: crate::traits::MediaKind,
    ) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::copy_media_uri_to_path(uri, dest_path)
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = uri;
            let _ = dest_path;
            Err(PlatformError::Platform(
                "copy_media_uri_to_path is only supported on iOS".to_string(),
            ))
        }
    }

    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        let data = super::resources::read_asset_data(path);

        if data.is_empty() {
            Err(PlatformError::AssetNotFound(path.to_string()))
        } else {
            Ok(Box::new(Cursor::new(data)))
        }
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        let entries = self.collect_files_recursively(asset_dir);
        Box::new(entries.into_iter())
    }

    fn exit_app(&self) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            use dispatch2::DispatchQueue;

            // Exit app on main thread
            DispatchQueue::main().exec_async(move || {
                // Force exit after a short delay to allow cleanup
                std::thread::sleep(std::time::Duration::from_millis(100));
                std::process::exit(0);
            });

            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            use objc2::rc::Retained;
            use objc2::{ClassType, extern_class, msg_send};
            use objc2_foundation::NSObject;

            extern_class!(
                #[derive(Debug, PartialEq, Eq, Hash)]
                #[unsafe(super(NSObject))]
                pub struct NSApplication;
            );

            impl NSApplication {
                pub fn shared() -> Retained<Self> {
                    unsafe { msg_send![Self::class(), sharedApplication] }
                }

                pub fn terminate(&self, sender: Option<&NSObject>) {
                    unsafe { msg_send![self, terminate: sender] }
                }
            }

            let app = NSApplication::shared();
            app.terminate(None);
            Ok(())
        }
    }

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn open_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError> {
        if ffi::open_lxapp(&appid, &path) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to open lxapp: appid={}, path={}",
                appid, path
            )))
        }
    }

    fn close_lxapp(&self, appid: String) -> Result<(), PlatformError> {
        if ffi::close_lxapp(&appid) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to close lxapp: appid={}",
                appid
            )))
        }
    }

    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: crate::traits::AnimationType,
    ) -> Result<(), PlatformError> {
        if ffi::navigate(&appid, &path, animation_type as i32) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to navigate: appid={}, path={}, animation_type={:?}",
                appid, path, animation_type
            )))
        }
    }

    fn launch_with_url(&self, url: String) -> Result<(), PlatformError> {
        ffi::launch_with_url(&url);
        Ok(())
    }
}

impl Platform {
    /// Recursively collect all files from a directory
    fn collect_files_recursively<'a>(
        &'a self,
        dir_path: &str,
    ) -> Vec<Result<AssetFileEntry<'a>, PlatformError>> {
        let mut all_files = Vec::new();
        let mut dirs_to_process = vec![dir_path.to_string()];

        while let Some(current_dir) = dirs_to_process.pop() {
            let contents = super::resources::list_asset_directory(&current_dir);

            for name in contents {
                let full_path = if current_dir.is_empty() || current_dir == "/" {
                    name.clone()
                } else {
                    format!("{}/{}", current_dir.trim_end_matches('/'), name)
                };

                // Try to read as file first
                let data = super::resources::read_asset_data(&full_path);

                if !data.is_empty() {
                    // It's a file, add it to results
                    let reader: Box<dyn Read + 'a> = Box::new(Cursor::new(data));
                    all_files.push(Ok(AssetFileEntry {
                        path: full_path,
                        reader,
                    }));
                } else {
                    // It might be a directory, try to list it
                    let sub_contents = super::resources::list_asset_directory(&full_path);
                    if !sub_contents.is_empty() {
                        // It's a directory with contents, add it to processing queue
                        dirs_to_process.push(full_path);
                    }
                }
            }
        }

        all_files
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use super::*;
    use std::fs;

    /// Copy media file from temporary location to application cache directory
    pub(super) fn copy_media_uri_to_path(
        uri: &str,
        dest_path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        let mut owned_uri: Option<String> = None;
        let resolved_uri = if let Some(stripped) = uri.strip_prefix("file://") {
            owned_uri = Some(stripped.to_string());
            owned_uri.as_deref().unwrap()
        } else {
            uri
        };

        let source_path = std::path::Path::new(resolved_uri);

        if !source_path.exists() {
            return Err(PlatformError::Platform(format!(
                "Source file does not exist: {}",
                uri
            )));
        }

        if let Some(parent) = dest_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(PlatformError::Platform(format!(
                    "Failed to create destination directory: {}",
                    e
                )));
            }
        }
        match fs::copy(source_path, dest_path) {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to copy file from {} to {}: {}",
                uri,
                dest_path.display(),
                e
            ))),
        }
    }
}
