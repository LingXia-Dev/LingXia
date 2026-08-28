use crate::error::PlatformError;
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileDialogResult,
};

// Async dialogs, deliberately: the sync `rfd::FileDialog` runs `runModal` on
// the main thread, which parks the whole app's runloop for as long as the
// panel is up — WebView JS callbacks stop being delivered, so every Logic
// eval in every lxapp times out until a human dismisses the panel. The async
// dialog presents the same panel through a completion-driven future instead,
// so the runloop (and everything on it) stays live while the user decides.

pub async fn choose_file_desktop(
    request: ChooseFileRequest,
) -> Result<FileDialogResult, PlatformError> {
    let dialog = apply_filters(
        apply_common_options(
            rfd::AsyncFileDialog::new(),
            &request.title,
            &request.default_path,
        ),
        &request.filters,
    );

    if request.multiple {
        match dialog.pick_files().await {
            Some(handles) => Ok(FileDialogResult {
                canceled: false,
                paths: handles
                    .iter()
                    .map(|handle| handle.path().to_string_lossy().into_owned())
                    .collect(),
            }),
            None => Ok(canceled()),
        }
    } else {
        match dialog.pick_file().await {
            Some(handle) => Ok(FileDialogResult {
                canceled: false,
                paths: vec![handle.path().to_string_lossy().into_owned()],
            }),
            None => Ok(canceled()),
        }
    }
}

pub async fn choose_directory_desktop(
    request: ChooseDirectoryRequest,
) -> Result<FileDialogResult, PlatformError> {
    let dialog = apply_common_options(
        rfd::AsyncFileDialog::new(),
        &request.title,
        &request.default_path,
    );
    match dialog.pick_folder().await {
        Some(handle) => Ok(FileDialogResult {
            canceled: false,
            paths: vec![handle.path().to_string_lossy().into_owned()],
        }),
        None => Ok(canceled()),
    }
}

fn canceled() -> FileDialogResult {
    FileDialogResult {
        canceled: true,
        paths: vec![],
    }
}

fn apply_common_options(
    mut dialog: rfd::AsyncFileDialog,
    title: &Option<String>,
    default_path: &Option<String>,
) -> rfd::AsyncFileDialog {
    if let Some(value) = title {
        dialog = dialog.set_title(value);
    }
    if let Some(value) = default_path {
        dialog = dialog.set_directory(value);
    }
    dialog
}

fn apply_filters(
    mut dialog: rfd::AsyncFileDialog,
    filters: &[FileDialogFilter],
) -> rfd::AsyncFileDialog {
    for filter in filters {
        if filter.extensions.is_empty() {
            continue;
        }
        let name = filter.name.as_deref().unwrap_or("Files");
        let exts: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter(name, &exts);
    }
    dialog
}
