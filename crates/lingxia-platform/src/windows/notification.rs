//! Local toasts: replace-by-tag, skip the banner when this process is frontmost.

use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::app_runtime::LocalNotificationShow;
use std::sync::Mutex;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::DateTime;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    NotificationSetting, ScheduledToastNotification, ToastNotification, ToastNotificationManager,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
    SetForegroundWindow,
};
use windows::core::BOOL;
use windows::core::HSTRING;

const TOAST_GROUP: &str = "lingxia.local";
const UNIX_MS_TO_WINRT: u64 = 11_644_473_600_000;

type ToastActivateHandler = fn(&str);
static TOAST_ACTIVATE_HANDLER: Mutex<Option<ToastActivateHandler>> = Mutex::new(None);

pub fn set_toast_activate_handler(handler: ToastActivateHandler) {
    if let Ok(mut slot) = TOAST_ACTIVATE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_toast_activate(args: &str) {
    if let Some(handler) = TOAST_ACTIVATE_HANDLER.lock().ok().and_then(|slot| *slot) {
        handler(args);
    }
    activate_host_windows();
}

pub(super) fn aumid_from_identity(identity: &str) -> String {
    identity
        .chars()
        .map(|c| if c.is_whitespace() { '.' } else { c })
        .take(127)
        .collect()
}

pub fn install_toast_identity(_platform: &Platform) {
    // Process AUMID is stamped in `install_taskbar_identity`. Unpackaged
    // toasts use that id; the Start Menu shortcut is created on first show
    // if the notifier reports Disabled for a missing shortcut.
}

pub(super) fn request_permission(platform: &Platform) -> Result<String, PlatformError> {
    ensure_start_menu_shortcut(platform);
    // Unpackaged Win32 often reports DisabledForApplication even when Show
    // still posts. Only treat explicit user / policy blocks as denied.
    match toast_setting(platform) {
        Ok(NotificationSetting::DisabledForUser)
        | Ok(NotificationSetting::DisabledByGroupPolicy) => Ok("denied".into()),
        Ok(_) => Ok("granted".into()),
        Err(_) => {
            if notifier(platform).is_ok() {
                Ok("granted".into())
            } else {
                Ok("denied".into())
            }
        }
    }
}

fn toast_setting(platform: &Platform) -> Result<NotificationSetting, PlatformError> {
    notifier(platform)?.Setting().map_err(win_err)
}

pub(super) fn show(
    platform: &Platform,
    request: &LocalNotificationShow,
) -> Result<String, PlatformError> {
    let now_ms = unix_now_ms();
    let scheduled = request.deliver_at_ms.filter(|at| *at > now_ms);
    // Immediate show is a no-op banner while this process is already in
    // front; do not require the toast identity just to resolve the id.
    if scheduled.is_none() && process_is_frontmost() {
        return Ok(request.id.clone());
    }
    let permission = request_permission(platform)?;
    if permission != "granted" {
        return Err(PlatformError::Platform(
            "notifications are disabled for this app in Windows Settings".into(),
        ));
    }

    let xml = toast_xml(
        &request.title,
        &request.body,
        request.applink.as_deref(),
        request.silent,
    );
    let document = XmlDocument::new().map_err(win_err)?;
    document.LoadXml(&HSTRING::from(xml)).map_err(win_err)?;

    let notifier = notifier(platform)?;
    let id = request.id.clone();
    if let Some(at_ms) = scheduled {
        cancel(platform, &id)?;
        let toast = ScheduledToastNotification::CreateScheduledToastNotification(
            &document,
            DateTime {
                UniversalTime: unix_ms_to_winrt(at_ms),
            },
        )
        .map_err(win_err)?;
        toast.SetTag(&HSTRING::from(&id)).map_err(win_err)?;
        toast
            .SetGroup(&HSTRING::from(TOAST_GROUP))
            .map_err(win_err)?;
        notifier.AddToSchedule(&toast).map_err(win_err)?;
    } else {
        let toast = ToastNotification::CreateToastNotification(&document).map_err(win_err)?;
        toast.SetTag(&HSTRING::from(&id)).map_err(win_err)?;
        toast
            .SetGroup(&HSTRING::from(TOAST_GROUP))
            .map_err(win_err)?;
        let applink = request.applink.clone().unwrap_or_default();
        toast
            .Activated(&TypedEventHandler::<ToastNotification, _>::new(
                move |_, _| {
                    if !applink.is_empty() {
                        invoke_toast_activate(&applink);
                    } else {
                        activate_host_windows();
                    }
                    Ok(())
                },
            ))
            .map_err(win_err)?;
        notifier.Show(&toast).map_err(win_err)?;
    }
    Ok(id)
}

pub(super) fn cancel(platform: &Platform, id: &str) -> Result<(), PlatformError> {
    let aumid = aumid_from_identity(&platform.autostart_value_name());
    if let Ok(history) = ToastNotificationManager::History() {
        let _ = history.RemoveGroupedTagWithId(
            &HSTRING::from(id),
            &HSTRING::from(TOAST_GROUP),
            &HSTRING::from(&aumid),
        );
    }
    if let Ok(notifier) = notifier(platform)
        && let Ok(scheduled) = notifier.GetScheduledToastNotifications()
    {
        let count = scheduled.Size().unwrap_or(0);
        for index in 0..count {
            if let Ok(toast) = scheduled.GetAt(index)
                && toast.Tag().ok().map(|s| s.to_string()).as_deref() == Some(id)
            {
                let _ = notifier.RemoveFromSchedule(&toast);
            }
        }
    }
    Ok(())
}

pub(super) fn cancel_all(platform: &Platform) -> Result<(), PlatformError> {
    let aumid = aumid_from_identity(&platform.autostart_value_name());
    if let Ok(history) = ToastNotificationManager::History() {
        let _ = history.RemoveGroupWithId(&HSTRING::from(TOAST_GROUP), &HSTRING::from(&aumid));
    }
    if let Ok(notifier) = notifier(platform)
        && let Ok(scheduled) = notifier.GetScheduledToastNotifications()
    {
        let count = scheduled.Size().unwrap_or(0);
        for index in 0..count {
            if let Ok(toast) = scheduled.GetAt(index)
                && toast.Group().ok().map(|s| s.to_string()).as_deref() == Some(TOAST_GROUP)
            {
                let _ = notifier.RemoveFromSchedule(&toast);
            }
        }
    }
    Ok(())
}

fn notifier(
    platform: &Platform,
) -> Result<windows::UI::Notifications::ToastNotifier, PlatformError> {
    if let Ok(notifier) = ToastNotificationManager::CreateToastNotifier() {
        return Ok(notifier);
    }
    let aumid = aumid_from_identity(&platform.autostart_value_name());
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid)).map_err(win_err)
}

fn ensure_start_menu_shortcut(platform: &Platform) {
    let aumid = aumid_from_identity(&platform.autostart_value_name());
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(programs) = programs_folder() else {
        return;
    };
    let stem: String = platform
        .product_name()
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '-'
            } else {
                c
            }
        })
        .collect();
    let link = programs.join(format!("{stem}.lnk"));
    if let Err(error) = write_aumid_shortcut(&link, &exe, &aumid) {
        log::warn!("failed to register toast Start Menu shortcut: {error}");
    }
}

fn programs_folder() -> Option<std::path::PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Programs, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    use windows::core::PWSTR;

    unsafe {
        let pwstr: PWSTR = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None).ok()?;
        let path = pwstr.to_string().ok().map(std::path::PathBuf::from);
        CoTaskMemFree(Some(pwstr.0.cast()));
        path
    }
}

fn write_aumid_shortcut(
    link: &std::path::Path,
    exe: &std::path::Path,
    aumid: &str,
) -> windows::core::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{Interface, PCWSTR};

    // {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, 5
    const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
        pid: 5,
    };

    let exe_wide: Vec<u16> = OsStrExt::encode_wide(exe.as_os_str())
        .chain(std::iter::once(0))
        .collect();
    let dir_wide: Option<Vec<u16>> = exe.parent().map(|dir| {
        OsStrExt::encode_wide(dir.as_os_str())
            .chain(std::iter::once(0))
            .collect()
    });
    let link_wide: Vec<u16> = OsStrExt::encode_wide(link.as_os_str())
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        shell.SetPath(PCWSTR(exe_wide.as_ptr()))?;
        if let Some(dir) = dir_wide.as_ref() {
            shell.SetWorkingDirectory(PCWSTR(dir.as_ptr()))?;
        }
        // Unpackaged toasts resolve through this shortcut's AppUserModel.ID.
        // Saving without it leaves CreateToastNotifierWithId orphaned.
        let store: IPropertyStore = shell.cast()?;
        let prop = PROPVARIANT::from(aumid);
        store.SetValue(&PKEY_APP_USER_MODEL_ID, &prop)?;
        store.Commit()?;
        let persist: IPersistFile = shell.cast()?;
        persist.Save(PCWSTR(link_wide.as_ptr()), true)?;
    }
    Ok(())
}

fn toast_xml(title: &str, body: &str, applink: Option<&str>, silent: bool) -> String {
    let launch = applink.unwrap_or("");
    let audio = if silent {
        r#"<audio silent="true"/>"#
    } else {
        ""
    };
    format!(
        r#"<toast launch="{}"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{}</toast>"#,
        xml_escape(launch),
        xml_escape(title),
        xml_escape(body),
        audio
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn unix_ms_to_winrt(ms: u64) -> i64 {
    ((ms.saturating_add(UNIX_MS_TO_WINRT)) * 10_000) as i64
}

fn win_err(error: windows::core::Error) -> PlatformError {
    PlatformError::Platform(error.to_string())
}

fn process_is_frontmost() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

fn activate_host_windows() {
    unsafe {
        let _ = EnumWindows(Some(activate_enum), LPARAM(0));
    }
}

unsafe extern "system" fn activate_enum(hwnd: HWND, _: LPARAM) -> BOOL {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() && IsWindowVisible(hwnd).as_bool() {
            let _ = SetForegroundWindow(hwnd);
        }
    }
    BOOL(1)
}
