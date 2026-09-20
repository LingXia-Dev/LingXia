//! Local toasts: replace-by-tag, skip the banner when this process is frontmost.
//!
//! Every tap — immediate, scheduled, or a notification-centre history item
//! long after this process exited — arrives through the COM activator in
//! [`super::toast_activator`], so the OS owns the schedule and the process
//! never has to still be running for a tap to find its target.

use super::app::Platform;
use super::toast_activator;
use crate::error::PlatformError;
use crate::traits::app_runtime::{LocalNotificationShow, LocalNotificationStatus};
use std::sync::Mutex;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::DateTime;
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

/// A toast was tapped. Brings the product forward first: a token that no
/// longer resolves still has to land the user on a visible product.
pub(super) fn on_toast_activated(args: &str) {
    activate_host_windows();
    let Some(token) = toast_activator::token_from_launch(args) else {
        log::debug!("ignoring a toast activation that is not ours: {args:?}");
        return;
    };
    if let Some(handler) = TOAST_ACTIVATE_HANDLER.lock().ok().and_then(|slot| *slot) {
        handler(token);
    }
}

pub(super) fn aumid_from_identity(identity: &str) -> String {
    identity
        .chars()
        .map(|c| if c.is_whitespace() { '.' } else { c })
        .take(127)
        .collect()
}

pub(super) fn permission(platform: &Platform) -> Result<String, PlatformError> {
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
) -> Result<LocalNotificationStatus, PlatformError> {
    let now_ms = unix_now_ms();
    let scheduled = request.deliver_at_ms.filter(|at| *at > now_ms + 500);
    let id = request.id.clone();
    if scheduled.is_none() && process_is_frontmost() {
        // Still an upsert: whatever the id held must not outlive this call.
        cancel(platform, &id)?;
        return Ok(LocalNotificationStatus::Suppressed);
    }
    if permission(platform)? != "granted" {
        return Err(PlatformError::Platform(
            "notifications are disabled for this app in Windows Settings".into(),
        ));
    }
    // Before anything is posted: a deployment that cannot take the tap back
    // must fail here rather than show a toast whose tap loses its target.
    toast_activator::ensure_registered(&aumid_from_identity(&platform.autostart_value_name()))?;
    cancel(platform, &id)?;

    let Some(at_ms) = scheduled else {
        show_now(platform, request)?;
        return Ok(LocalNotificationStatus::Posted);
    };

    let toast = ScheduledToastNotification::CreateScheduledToastNotification(
        &toast_document(request)?,
        DateTime {
            UniversalTime: unix_ms_to_winrt(at_ms),
        },
    )
    .map_err(win_err)?;
    toast.SetTag(&HSTRING::from(&id)).map_err(win_err)?;
    toast
        .SetGroup(&HSTRING::from(TOAST_GROUP))
        .map_err(win_err)?;
    toast
        .SetId(&HSTRING::from(schedule_id()))
        .map_err(win_err)?;
    // The OS owns the schedule from here: nothing takes it back early, so an
    // exit before it is due no longer drops it.
    notifier(platform)?.AddToSchedule(&toast).map_err(win_err)?;
    Ok(LocalNotificationStatus::Scheduled)
}

fn toast_document(request: &LocalNotificationShow) -> Result<XmlDocument, PlatformError> {
    let xml = toast_xml(
        &request.title,
        &request.body,
        &toast_activator::launch_payload(&request.activation_token),
        request.silent,
    );
    let document = XmlDocument::new().map_err(win_err)?;
    document.LoadXml(&HSTRING::from(xml)).map_err(win_err)?;
    Ok(document)
}

fn show_now(platform: &Platform, request: &LocalNotificationShow) -> Result<(), PlatformError> {
    let toast =
        ToastNotification::CreateToastNotification(&toast_document(request)?).map_err(win_err)?;
    toast.SetTag(&HSTRING::from(&request.id)).map_err(win_err)?;
    toast
        .SetGroup(&HSTRING::from(TOAST_GROUP))
        .map_err(win_err)?;
    // No in-process Activated handler: this toast outlives the process in the
    // notification centre, and one activation path has to serve both.
    notifier(platform)?.Show(&toast).map_err(win_err)
}

/// `ScheduledToastNotification.Id` rejects 16 characters (0x803E0120); eight
/// hex digits of a per-process counter are unique for as long as we run. It
/// only has to be unique among pending schedules — the tag is the replace key
/// and the `launch` token is what a tap resolves.
fn schedule_id() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!(
        "{:08x}",
        (unix_now_ms() as u32).wrapping_add(sequence.wrapping_mul(0x9E37_79B9))
    )
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
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| write_start_menu_shortcut(platform));
}

/// Register the COM activator so a cold tap can reach this product.
///
/// Called from the host bootstrap when the product declared notifications:
/// COM starts the exe for a tap and then waits for the class object, which a
/// registration deferred to the first `show` would never reach.
pub fn ensure_toast_activator(platform: &Platform) -> Result<(), PlatformError> {
    ensure_start_menu_shortcut(platform);
    toast_activator::ensure_registered(&aumid_from_identity(&platform.autostart_value_name()))
}

/// Remove this product's toast registration. For an uninstaller.
pub fn remove_toast_registration(platform: &Platform) {
    let aumid = aumid_from_identity(&platform.autostart_value_name());
    toast_activator::unregister_toast_activator(&aumid);
    if let Some(link) = start_menu_link(platform) {
        let _ = std::fs::remove_file(link);
    }
}

fn start_menu_link(platform: &Platform) -> Option<std::path::PathBuf> {
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
    Some(programs_folder()?.join(format!("{stem}.lnk")))
}

fn write_start_menu_shortcut(platform: &Platform) {
    let aumid = aumid_from_identity(&platform.autostart_value_name());
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(link) = start_menu_link(platform) else {
        return;
    };
    // Rewritten every run, including over an installer's shortcut: the whole
    // file is derived from the exe path, the AUMID and the activator CLSID, so
    // this is how a shortcut written before the activator existed — or one
    // left behind by a moved exe — gets fixed.
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
    // Same fmtid, pid 26: System.AppUserModel.ToastActivatorCLSID. Without it
    // the shell has no way to start us for a tap, and the toast is inert.
    const PKEY_TOAST_ACTIVATOR_CLSID: PROPERTYKEY = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
        pid: 26,
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
        let activator = clsid_propvariant(toast_activator::clsid_for(aumid))?;
        store.SetValue(&PKEY_TOAST_ACTIVATOR_CLSID, &activator)?;
        store.Commit()?;
        let persist: IPersistFile = shell.cast()?;
        persist.Save(PCWSTR(link_wide.as_ptr()), true)?;
    }
    Ok(())
}

/// A `VT_CLSID` value. `propvarutil.h` spells this inline, so there is no
/// exported helper to call: the GUID has to be task-allocated, because
/// `PropVariantClear` frees it when the value drops.
fn clsid_propvariant(
    clsid: windows::core::GUID,
) -> windows::core::Result<windows::Win32::System::Com::StructuredStorage::PROPVARIANT> {
    use windows::Win32::Foundation::E_OUTOFMEMORY;
    use windows::Win32::System::Com::CoTaskMemAlloc;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Variant::VT_CLSID;

    unsafe {
        let slot = CoTaskMemAlloc(std::mem::size_of::<windows::core::GUID>())
            .cast::<windows::core::GUID>();
        if slot.is_null() {
            return Err(E_OUTOFMEMORY.into());
        }
        slot.write(clsid);
        let mut value = PROPVARIANT::default();
        let inner = &mut *value.Anonymous.Anonymous;
        inner.vt = VT_CLSID;
        inner.Anonymous.puuid = slot;
        Ok(value)
    }
}

fn toast_xml(title: &str, body: &str, launch: &str, silent: bool) -> String {
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
