use crate::error::PlatformError;
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileDialogResult,
};

pub async fn choose_file_desktop(
    request: ChooseFileRequest,
) -> Result<FileDialogResult, PlatformError> {
    let handle = crate::bg_runtime::spawn_blocking(move || run_file_dialog(&request));
    match handle {
        Some(h) => h
            .await
            .map_err(|e| PlatformError::Platform(format!("choose_file task panicked: {}", e)))?,
        None => Err(PlatformError::Platform(
            "choose_file: async runtime not initialized".into(),
        )),
    }
}

pub async fn choose_directory_desktop(
    request: ChooseDirectoryRequest,
) -> Result<FileDialogResult, PlatformError> {
    let handle = crate::bg_runtime::spawn_blocking(move || run_directory_dialog(&request));
    match handle {
        Some(h) => h.await.map_err(|e| {
            PlatformError::Platform(format!("choose_directory task panicked: {}", e))
        })?,
        None => Err(PlatformError::Platform(
            "choose_directory: async runtime not initialized".into(),
        )),
    }
}

fn apply_common_options(
    mut dialog: rfd::FileDialog,
    title: &Option<String>,
    default_path: &Option<String>,
) -> rfd::FileDialog {
    if let Some(value) = title {
        dialog = dialog.set_title(value);
    }
    if let Some(value) = default_path {
        dialog = dialog.set_directory(value);
    }
    dialog
}

fn apply_filters(mut dialog: rfd::FileDialog, filters: &[FileDialogFilter]) -> rfd::FileDialog {
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

fn run_file_dialog(request: &ChooseFileRequest) -> Result<FileDialogResult, PlatformError> {
    let dialog = apply_common_options(
        rfd::FileDialog::new(),
        &request.title,
        &request.default_path,
    );
    let dialog = apply_filters(dialog, &request.filters);

    if request.multiple {
        match dialog.pick_files() {
            Some(paths) => Ok(FileDialogResult {
                canceled: false,
                paths: paths
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
            }),
            None => Ok(FileDialogResult {
                canceled: true,
                paths: vec![],
            }),
        }
    } else {
        match dialog.pick_file() {
            Some(path) => Ok(FileDialogResult {
                canceled: false,
                paths: vec![path.to_string_lossy().into_owned()],
            }),
            None => Ok(FileDialogResult {
                canceled: true,
                paths: vec![],
            }),
        }
    }
}

fn run_directory_dialog(
    request: &ChooseDirectoryRequest,
) -> Result<FileDialogResult, PlatformError> {
    let dialog = apply_common_options(
        rfd::FileDialog::new(),
        &request.title,
        &request.default_path,
    );
    match dialog.pick_folder() {
        Some(path) => Ok(FileDialogResult {
            canceled: false,
            paths: vec![path.to_string_lossy().into_owned()],
        }),
        None => Ok(FileDialogResult {
            canceled: true,
            paths: vec![],
        }),
    }
}
