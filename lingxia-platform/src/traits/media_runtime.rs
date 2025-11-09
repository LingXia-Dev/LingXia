use std::path::{Path, PathBuf};

use crate::error::PlatformError;

use super::media_interaction::MediaKind;

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompressImageRequest {
    pub source_uri: String,
    pub quality: u8,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub output_path: PathBuf,
}

pub trait MediaRuntime: Send + Sync + 'static {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError>;

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError>;

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError>;
}
