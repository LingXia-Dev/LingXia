use miniapp::{AssetFileEntry, DeviceInfo, MiniAppError};
use std::path::PathBuf;
use std::io::Read;

#[derive(Clone)]
pub struct App {
    data_dir: String,
    cache_dir: String,
}

unsafe impl Send for App {}
unsafe impl Sync for App {}

impl App {
    pub fn new(data_dir: String, cache_dir: String) -> Result<Self, String> {
        Ok(App {
            data_dir,
            cache_dir,
        })
    }

    pub fn read_asset<'a>(&'a self, _path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError> {
        todo!()
    }

    pub fn asset_dir_iter<'a>(
        &'a self,
        _asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a> {
        todo!()
    }

    pub fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    pub fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    pub fn device_info(&self) -> DeviceInfo {
        todo!()
    }

    pub fn open_miniapp(&self, _appid: &str, _path: &str) -> Result<(), MiniAppError> {
        Ok(())
    }

    pub fn close_miniapp(&self, _appid: &str) -> Result<(), MiniAppError> {
        Ok(())
    }

    pub fn switch_page(&self, _appid: &str, _path: &str) -> Result<(), MiniAppError> {
        Ok(())
    }
}
