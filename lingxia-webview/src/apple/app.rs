use miniapp::{AssetFileEntry, DeviceInfo, MiniAppError};
use std::io::{Cursor, Read};
use std::path::PathBuf;

/// Shared App structure between Rust and Swift
#[derive(Clone)]
pub struct App {
    pub data_dir: String,
    pub cache_dir: String,
}

unsafe impl Send for App {}
unsafe impl Sync for App {}

impl App {
    /// Create a new App instance
    pub fn new(data_dir: String, cache_dir: String) -> Result<Self, MiniAppError> {
        Ok(App {
            data_dir,
            cache_dir,
        })
    }

    /// Get the data directory path
    pub fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    /// Get the cache directory path
    pub fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    /// Read an asset file from the SPM bundle resources
    pub fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError> {
        log::debug!("Reading asset: {}", path);

        // Call Swift function to read asset data
        let data = super::ffi::read_asset_data(path);

        if data.is_empty() {
            log::error!("Asset not found or failed to read: {}", path);
            Err(MiniAppError::ResourceNotFound(path.to_string()))
        } else {
            log::debug!(
                "Successfully read {} bytes from asset: {}",
                data.len(),
                path
            );
            Ok(Box::new(Cursor::new(data)))
        }
    }

    /// Iterate over files in an asset directory
    pub fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a> {
        log::debug!("Listing asset directory: {}", asset_dir);

        // Get directory contents from Swift
        let contents = super::ffi::list_asset_directory(asset_dir);

        let base_path = asset_dir.to_string();
        let entries: Vec<Result<AssetFileEntry<'a>, MiniAppError>> = contents
            .into_iter()
            .map(move |name| {
                let full_path = if base_path.is_empty() || base_path == "/" {
                    name.clone()
                } else {
                    format!("{}/{}", base_path.trim_end_matches('/'), name)
                };

                // Create a reader for the file
                let data = super::ffi::read_asset_data(&full_path);
                let reader: Box<dyn Read + 'a> = Box::new(Cursor::new(data));

                Ok(AssetFileEntry {
                    path: full_path,
                    reader,
                })
            })
            .collect();

        Box::new(entries.into_iter())
    }

    /// Get device information
    pub fn device_info(&self) -> DeviceInfo {
        todo!()
    }

    /// Open a mini app
    pub fn open_miniapp(&self, _appid: &str, _path: &str) -> Result<(), MiniAppError> {
        Ok(())
    }

    /// Close a mini app
    pub fn close_miniapp(&self, _appid: &str) -> Result<(), MiniAppError> {
        Ok(())
    }

    /// Switch to a page in a mini app
    pub fn switch_page(&self, _appid: &str, _path: &str) -> Result<(), MiniAppError> {
        Ok(())
    }
}
