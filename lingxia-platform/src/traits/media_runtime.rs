use std::path::Path;

use crate::error::PlatformError;

use super::media_interaction::MediaKind;

pub trait MediaRuntime: Send + Sync + 'static {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError>;
}
