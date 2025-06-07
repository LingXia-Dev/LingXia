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
        // Call Swift function to read asset data
        let data = super::ffi::read_asset_data(path);

        if data.is_empty() {
            Err(MiniAppError::ResourceNotFound(path.to_string()))
        } else {
            Ok(Box::new(Cursor::new(data)))
        }
    }

    /// Iterate over files in an asset directory
    pub fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a> {
        let entries = self.collect_files_recursively(asset_dir);
        Box::new(entries.into_iter())
    }

    /// Recursively collect all files from a directory
    fn collect_files_recursively<'a>(
        &'a self,
        dir_path: &str,
    ) -> Vec<Result<AssetFileEntry<'a>, MiniAppError>> {
        let mut all_files = Vec::new();
        let mut dirs_to_process = vec![dir_path.to_string()];

        while let Some(current_dir) = dirs_to_process.pop() {
            // Get directory contents from Swift
            let contents = super::ffi::list_asset_directory(&current_dir);

            for name in contents {
                let full_path = if current_dir.is_empty() || current_dir == "/" {
                    name.clone()
                } else {
                    format!("{}/{}", current_dir.trim_end_matches('/'), name)
                };

                // Try to read as file first
                let data = super::ffi::read_asset_data(&full_path);

                if !data.is_empty() {
                    // It's a file, add it to results
                    let reader: Box<dyn Read + 'a> = Box::new(Cursor::new(data));
                    all_files.push(Ok(AssetFileEntry {
                        path: full_path,
                        reader,
                    }));
                } else {
                    // It might be a directory, try to list it
                    let sub_contents = super::ffi::list_asset_directory(&full_path);
                    if !sub_contents.is_empty() {
                        // It's a directory with contents, add it to processing queue
                        dirs_to_process.push(full_path);
                    }
                }
            }
        }

        all_files
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
