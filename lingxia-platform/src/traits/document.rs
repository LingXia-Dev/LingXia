use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct OpenDocumentRequest {
    pub file_path: String,
    pub mime_type: Option<String>,
    pub show_menu: Option<bool>,
}

pub trait DocumentInteraction: Send + Sync + 'static {
    fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError>;
}
