use std::future::Future;
use std::path::Path;

use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct OpenFileRequest {
    pub path: String,
    pub mime_type: Option<String>,
    pub show_menu: Option<bool>,
}

impl OpenFileRequest {
    pub fn is_pdf_like(&self) -> bool {
        self.mime_type
            .as_deref()
            .map(|mime| mime.eq_ignore_ascii_case("application/pdf"))
            .unwrap_or(false)
            || Path::new(&self.path)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct RevealInFileManagerRequest {
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct FileDialogFilter {
    pub name: Option<String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PickFileRequest {
    pub multiple: bool,
    pub filters: Vec<FileDialogFilter>,
    pub title: Option<String>,
    pub default_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PickDirectoryRequest {
    pub title: Option<String>,
    pub default_path: Option<String>,
}

/// One picked item. `name` is the platform display name when the picker
/// reports one; `path` may be an opaque reference whose tail is not a name.
#[derive(Debug, Clone)]
pub struct PickedPath {
    pub path: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileDialogResult {
    pub canceled: bool,
    pub entries: Vec<PickedPath>,
}

impl FileDialogResult {
    pub fn canceled() -> Self {
        Self {
            canceled: true,
            entries: Vec::new(),
        }
    }

    /// Builds a completed result from a native payload: `names` is parallel to
    /// `paths` when present. A length mismatch is a native bug, not a missing
    /// name, so it is an error rather than being truncated.
    pub fn from_wire(
        canceled: bool,
        paths: Vec<String>,
        names: Option<Vec<Option<String>>>,
    ) -> Result<Self, PlatformError> {
        let names = match names {
            Some(names) if names.len() == paths.len() => names,
            Some(names) => {
                return Err(PlatformError::Platform(format!(
                    "file dialog returned {} paths but {} names",
                    paths.len(),
                    names.len()
                )));
            }
            None => vec![None; paths.len()],
        };
        Ok(Self {
            canceled,
            entries: paths
                .into_iter()
                .zip(names)
                .map(|(path, name)| PickedPath { path, name })
                .collect(),
        })
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.path.as_str())
    }
}

pub trait FileService: Send + Sync + 'static {
    fn review_file(
        &self,
        _request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send;

    fn open_external(
        &self,
        _request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "open_external is not supported on this platform".into(),
            ))
        }
    }

    fn reveal_in_file_manager(
        &self,
        _request: RevealInFileManagerRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "reveal_in_file_manager is not supported on this platform".into(),
            ))
        }
    }

    fn pick_file(
        &self,
        _request: PickFileRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "pick_file is not supported on this platform".into(),
            ))
        }
    }

    fn pick_directory(
        &self,
        _request: PickDirectoryRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "pick_directory is not supported on this platform".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Vec<String> {
        vec!["/a/one.txt".into(), "/b/two.txt".into()]
    }

    #[test]
    fn from_wire_zips_names_with_paths() {
        let result =
            FileDialogResult::from_wire(false, paths(), Some(vec![Some("One.txt".into()), None]))
                .unwrap();
        assert_eq!(result.entries[0].name.as_deref(), Some("One.txt"));
        assert_eq!(result.entries[1].path, "/b/two.txt");
        assert_eq!(result.entries[1].name, None);
    }

    #[test]
    fn from_wire_without_names_leaves_every_name_unset() {
        let result = FileDialogResult::from_wire(false, paths(), None).unwrap();
        assert!(result.entries.iter().all(|entry| entry.name.is_none()));
    }

    #[test]
    fn from_wire_rejects_a_names_length_mismatch() {
        assert!(FileDialogResult::from_wire(false, paths(), Some(vec![None])).is_err());
    }

    #[test]
    fn canceled_has_no_entries() {
        let result = FileDialogResult::canceled();
        assert!(result.canceled && result.entries.is_empty());
    }
}
