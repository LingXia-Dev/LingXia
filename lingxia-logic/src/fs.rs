use lingxia_platform::traits::document::{DocumentInteraction, OpenDocumentRequest};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};

#[derive(FromJSObj)]
struct JSOpenDocumentOptions {
    #[rename = "filePath"]
    file_path: String,
    #[rename = "fileType"]
    file_type: Option<String>,
    /// Controls share/menu button visibility
    /// - For PDF: Works on all platforms (Android, iOS, Harmony)
    /// - For Office docs (Word/Excel/PPT) and other files (ZIP, etc.):
    ///   * All platforms: No effect (always opens with default system app)
    #[rename = "showMenu"]
    show_menu: Option<bool>,
}

/// Maps file type string to appropriate MIME type
fn map_file_type_to_mime(file_type: Option<String>) -> Option<String> {
    match file_type.unwrap_or_default().to_lowercase().as_str() {
        "pdf" => Some("application/pdf".to_string()),
        "doc" => Some("application/msword".to_string()),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        ),
        "ppt" => Some("application/vnd.ms-powerpoint".to_string()),
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string(),
        ),
        "xls" => Some("application/vnd.ms-excel".to_string()),
        "xlsx" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string())
        }
        "zip" => Some("application/zip".to_string()),
        _ => None, // Let the system auto-detect
    }
}

fn open_document(ctx: JSContext, options: JSOpenDocumentOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    if options.file_path.is_empty() {
        return Err(RongJSError::Error("openDocument requires filePath".into()));
    }

    // Resolve the file path to ensure it's accessible
    let resolved_path = lxapp
        .resolve_accessible_path(&options.file_path)
        .map_err(|err| RongJSError::Error(format!("openDocument path not accessible: {}", err)))?;
    let normalized_path = resolved_path.to_string_lossy().into_owned();

    let mime_type = map_file_type_to_mime(options.file_type);

    let request = OpenDocumentRequest {
        file_path: normalized_path,
        mime_type,
        show_menu: options.show_menu,
    };

    runtime
        .open_document(request)
        .map_err(|e| RongJSError::Error(format!("openDocument failed: {}", e)))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let open_document_func = JSFunc::new(ctx, open_document)?;
    lx::register_js_api(ctx, "openDocument", open_document_func)?;

    Ok(())
}
