use super::ffi;
use crate::error::PlatformError;
use crate::traits::{PermissionKind, PermissionStatus, Permissions};
use crate::{AppRuntime, AssetFileEntry, MediaRuntime};
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

impl crate::traits::UpdateService for Platform {}

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

impl Permissions for Platform {
    fn check_permission(
        &self,
        permission: PermissionKind,
    ) -> Result<PermissionStatus, PlatformError> {
        #[cfg(target_os = "ios")]
        {
            use crate::traits::PermissionStatus as Status;

            match permission {
                PermissionKind::Location => {
                    // Mirror the logic in the iOS location module.
                    use crate::apple::location::ios;
                    let enabled = ios::is_location_enabled()?;
                    if !enabled {
                        return Ok(Status::Denied);
                    }
                    let auth = ios::current_authorization_status();
                    let status = match auth {
                        ios::AuthorizationState::Authorized => Status::Granted,
                        ios::AuthorizationState::Denied => Status::Denied,
                        ios::AuthorizationState::Restricted => Status::Restricted,
                        ios::AuthorizationState::NotDetermined => Status::Unknown,
                    };
                    Ok(status)
                }
                // Other permissions are currently handled at the UI layer on iOS.
                _ => Ok(PermissionStatus::Unknown),
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = permission;
            Ok(PermissionStatus::Unknown)
        }
    }

    fn request_permission(
        &self,
        _permission: PermissionKind,
        _callback_id: u64,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "Explicit permission requests are handled in the iOS UI layer".to_string(),
        ))
    }
}

impl AppRuntime for Platform {
    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn get_app_identifier(&self) -> Result<String, PlatformError> {
        use objc2_foundation::NSBundle;
        let bundle = NSBundle::mainBundle();
        if let Some(identifier) = bundle.bundleIdentifier() {
            Ok(identifier.to_string())
        } else {
            Err(PlatformError::Platform(
                "Failed to get bundle identifier".to_string(),
            ))
        }
    }

    /// Copy album media to a local file path
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &std::path::Path,
        kind: crate::traits::MediaKind,
    ) -> Result<(), PlatformError> {
        MediaRuntime::copy_album_media_to_file(self, uri, dest_path, kind)
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

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn show_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError> {
        if ffi::open_lxapp(&appid, &path) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to show lxapp: appid={}, path={}",
                appid, path
            )))
        }
    }

    fn hide_lxapp(&self, appid: String) -> Result<(), PlatformError> {
        if ffi::close_lxapp(&appid) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to hide lxapp: appid={}",
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

    fn get_capsule_rect(&self, _callback_id: u64) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ffi::get_capsule_rect(_callback_id);
            Ok(())
        }
        #[cfg(not(target_os = "ios"))]
        {
            Err(PlatformError::Platform(
                "getCapsuleRect is only supported on iOS".to_string(),
            ))
        }
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
