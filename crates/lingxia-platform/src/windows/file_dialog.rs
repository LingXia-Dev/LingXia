//! System open dialog (`IFileOpenDialog`), called directly rather than through
//! rfd: rfd reports a failed dialog as a cancel and drops the file-must-exist
//! check from multi-select.

use std::path::PathBuf;

use lingxia_webview::WebTag;
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST,
    FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, IsWindowVisible};
use windows::core::{HSTRING, PCWSTR};

use crate::error::PlatformError;

pub(crate) enum DialogKind {
    File { multiple: bool },
    Folder,
}

pub(crate) struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

pub(crate) struct OpenDialog {
    pub kind: DialogKind,
    pub title: Option<String>,
    pub default_path: Option<String>,
    pub filters: Vec<DialogFilter>,
    /// Key of the WebView whose window owns the dialog.
    pub owner: Option<String>,
}

/// Shows the dialog and resolves the chosen paths, or `None` on cancel.
///
/// Runs on its own thread: the dialog needs an STA, and a pooled thread may
/// already be in the MTA (Media Foundation joins it).
pub(crate) async fn show(dialog: OpenDialog) -> Result<Option<Vec<PathBuf>>, PlatformError> {
    let owner = owner_window(dialog.owner.as_deref()).map(|hwnd| hwnd.0 as isize);
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("lx-file-dialog".to_string())
        .spawn(move || {
            let owner = owner.map(|raw| HWND(raw as *mut _));
            let _ = tx.send(show_on_sta(&dialog, owner));
        })
        .map_err(|err| PlatformError::Platform(format!("failed to start file dialog: {err}")))?;
    rx.await
        .map_err(|_| PlatformError::Platform("file dialog thread exited".to_string()))?
}

fn owner_window(owner: Option<&str>) -> Option<HWND> {
    owner
        .and_then(|key| lingxia_windows_contract::webview_window_snapshot(&WebTag::from(key)).ok())
        .map(|snapshot| unsafe { GetAncestor(HWND(snapshot.window_id as *mut _), GA_ROOT) })
        .filter(|hwnd| !hwnd.is_invalid() && unsafe { IsWindowVisible(*hwnd).as_bool() })
        .or_else(super::update_card::find_main_window)
}

fn show_on_sta(
    dialog: &OpenDialog,
    owner: Option<HWND>,
) -> Result<Option<Vec<PathBuf>>, PlatformError> {
    let initialized =
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }.is_ok();
    let result = run(dialog, owner).map_err(|err| {
        PlatformError::Platform(format!("system file dialog failed: {}", err.message()))
    });
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

fn run(dialog: &OpenDialog, owner: Option<HWND>) -> windows::core::Result<Option<Vec<PathBuf>>> {
    unsafe {
        let picker: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;

        let mut options = picker.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
        match dialog.kind {
            DialogKind::Folder => options |= FOS_PICKFOLDERS,
            DialogKind::File { multiple } => {
                options |= FOS_FILEMUSTEXIST;
                if multiple {
                    options |= FOS_ALLOWMULTISELECT;
                }
            }
        }
        picker.SetOptions(options)?;

        if let Some(title) = &dialog.title {
            picker.SetTitle(&HSTRING::from(title.as_str()))?;
        }

        // A default folder that does not resolve is only a hint; open anyway.
        if let Some(path) = &dialog.default_path {
            match SHCreateItemFromParsingName::<_, _, IShellItem>(
                &HSTRING::from(path.replace('/', "\\")),
                None,
            ) {
                Ok(folder) => picker.SetFolder(&folder)?,
                Err(err) => log::warn!("file dialog default path {path} ignored: {err}"),
            }
        }

        // COMDLG_FILTERSPEC borrows these buffers until Show returns.
        let specs: Vec<(HSTRING, HSTRING)> = dialog
            .filters
            .iter()
            .filter(|filter| !filter.extensions.is_empty())
            .map(|filter| {
                let pattern = filter
                    .extensions
                    .iter()
                    .map(|ext| format!("*.{ext}"))
                    .collect::<Vec<_>>()
                    .join(";");
                (HSTRING::from(filter.name.as_str()), HSTRING::from(pattern))
            })
            .collect();
        if !specs.is_empty() {
            let raw: Vec<COMDLG_FILTERSPEC> = specs
                .iter()
                .map(|(name, pattern)| COMDLG_FILTERSPEC {
                    pszName: PCWSTR(name.as_ptr()),
                    pszSpec: PCWSTR(pattern.as_ptr()),
                })
                .collect();
            picker.SetFileTypes(&raw)?;
        }

        if let Err(err) = picker.Show(owner) {
            if err.code() == ERROR_CANCELLED.to_hresult() {
                return Ok(None);
            }
            return Err(err);
        }

        let items = picker.GetResults()?;
        let mut paths = Vec::new();
        for index in 0..items.GetCount()? {
            paths.push(item_path(&items.GetItemAt(index)?)?);
        }
        Ok(Some(paths))
    }
}

fn item_path(item: &IShellItem) -> windows::core::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    unsafe {
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = std::ffi::OsString::from_wide(raw.as_wide());
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(PathBuf::from(path))
    }
}
