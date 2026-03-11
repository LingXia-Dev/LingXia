use crate::error::PlatformError;
use crate::traits::file::{ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter};

pub fn choose_file_desktop(request: ChooseFileRequest) -> Result<(), PlatformError> {
    let _ = crate::bg_runtime::spawn_blocking(move || {
        let callback_id = request.callback_id;
        match run_file_dialog(&request) {
            Ok((canceled, paths)) => send_success(callback_id, canceled, paths),
            Err(e) => send_error(callback_id, e),
        }
    });

    Ok(())
}

pub fn choose_directory_desktop(request: ChooseDirectoryRequest) -> Result<(), PlatformError> {
    let _ = crate::bg_runtime::spawn_blocking(move || {
        let callback_id = request.callback_id;
        match run_directory_dialog(&request) {
            Ok((canceled, paths)) => send_success(callback_id, canceled, paths),
            Err(e) => send_error(callback_id, e),
        }
    });

    Ok(())
}

fn send_success(callback_id: u64, canceled: bool, paths: Vec<String>) {
    let payload = serde_json::json!({ "canceled": canceled, "paths": paths }).to_string();
    let _ = lingxia_messaging::invoke_callback(callback_id, Ok(payload));
}

fn send_error(callback_id: u64, error: PlatformError) {
    log::error!("file dialog error: {}", error);
    let _ = lingxia_messaging::invoke_callback(callback_id, Err(1002));
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

fn run_file_dialog(request: &ChooseFileRequest) -> Result<(bool, Vec<String>), PlatformError> {
    let dialog = apply_common_options(
        rfd::FileDialog::new(),
        &request.title,
        &request.default_path,
    );
    let dialog = apply_filters(dialog, &request.filters);

    if request.multiple {
        match dialog.pick_files() {
            Some(paths) => Ok((
                false,
                paths
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
            )),
            None => Ok((true, vec![])),
        }
    } else {
        match dialog.pick_file() {
            Some(path) => Ok((false, vec![path.to_string_lossy().into_owned()])),
            None => Ok((true, vec![])),
        }
    }
}

fn run_directory_dialog(
    request: &ChooseDirectoryRequest,
) -> Result<(bool, Vec<String>), PlatformError> {
    let dialog = apply_common_options(
        rfd::FileDialog::new(),
        &request.title,
        &request.default_path,
    );
    match dialog.pick_folder() {
        Some(path) => Ok((false, vec![path.to_string_lossy().into_owned()])),
        None => Ok((true, vec![])),
    }
}
