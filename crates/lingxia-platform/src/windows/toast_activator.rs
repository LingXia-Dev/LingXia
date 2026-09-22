//! Desktop toast COM activator.
//!
//! An unpackaged Win32 toast only reaches its process through
//! `INotificationActivationCallback`. Without it a scheduled toast, or any
//! toast tapped from the notification centre after the process exited, reaches
//! nobody — which is why a tap used to need the process to still be running.
//!
//! Three things have to line up, and all three are per-user:
//!
//! 1. the Start Menu shortcut carries the AUMID and this CLSID;
//! 2. `HKCU\SOFTWARE\Classes\CLSID\{clsid}\LocalServer32` names the exe, so
//!    COM can start it for a cold tap;
//! 3. the running process has a class factory registered for the CLSID.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_NOINTERFACE, E_POINTER};
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED, CoInitializeEx, CoRegisterClassObject,
    CoRevokeClassObject, IClassFactory, IClassFactory_Impl, REGCLS_MULTIPLEUSE,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::Notifications::{
    INotificationActivationCallback, INotificationActivationCallback_Impl,
    NOTIFICATION_USER_INPUT_DATA,
};
use windows::core::{BOOL, GUID, IUnknown, Interface, PCWSTR, Ref, Result as WinResult, implement};

use crate::error::PlatformError;

static REGISTRATION: AtomicU32 = AtomicU32::new(0);

/// A stable per-product CLSID, derived from the AUMID.
///
/// Deriving it keeps products from having to mint and carry a GUID whose only
/// job is to be different from the next product's. Two AUMIDs that differ at
/// all produce different CLSIDs; the same AUMID always produces the same one,
/// which is what an installed shortcut and a registry key both depend on.
pub(super) fn clsid_for(aumid: &str) -> GUID {
    let low = fnv1a64(aumid.as_bytes(), 0xcbf2_9ce4_8422_2325);
    let high = fnv1a64(aumid.as_bytes(), 0x9e37_79b9_7f4a_7c15);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&low.to_be_bytes());
    bytes[8..].copy_from_slice(&high.to_be_bytes());
    // Shape it like a name-based UUID so nothing mistakes it for a random one.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    GUID::from_values(
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u16::from_be_bytes([bytes[4], bytes[5]]),
        u16::from_be_bytes([bytes[6], bytes[7]]),
        [
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ],
    )
}

fn fnv1a64(bytes: &[u8], offset: u64) -> u64 {
    let mut hash = offset;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub(super) fn launch_payload(activation_token: &str) -> String {
    crate::traits::app_runtime::wrap_activation(activation_token)
}

/// The token a `launch` payload carries, or `None` when it is not ours.
pub(super) fn token_from_launch(args: &str) -> Option<&str> {
    crate::traits::app_runtime::unwrap_activation(args)
}

/// Register the class factory and the LocalServer32 key. Idempotent.
///
/// Runs at startup for a product that declared notifications, because COM
/// starts the exe for a cold tap and then waits for the class object — a
/// registration deferred to the first `show` would never happen on that run.
/// `show` calls it again so a host that skipped startup registration fails its
/// publish instead of posting a toast nothing can activate.
///
/// The registration lives on a thread of its own that enters the MTA and
/// parks: the class table entry is only as durable as the apartment, and the
/// callers here are pool threads that come and go.
pub(super) fn ensure_registered(aumid: &str) -> Result<(), PlatformError> {
    static ONCE: std::sync::Once = std::sync::Once::new();
    static OUTCOME: Mutex<Option<Result<(), String>>> = Mutex::new(None);

    ONCE.call_once(|| {
        let clsid = clsid_for(aumid);
        let outcome = write_local_server(&clsid).map_err(|error| error.to_string());
        let outcome = outcome.and_then(|()| {
            let (registered, ready) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("lingxia-toast-activator".into())
                .spawn(move || {
                    // Ignore the result: an apartment this thread joins rather
                    // than creates is just as good, and the register call below
                    // reports anything that actually went wrong.
                    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                    let factory: IClassFactory = Factory.into();
                    let outcome = unsafe {
                        CoRegisterClassObject(
                            &clsid,
                            &factory,
                            CLSCTX_LOCAL_SERVER,
                            REGCLS_MULTIPLEUSE,
                        )
                    };
                    match outcome {
                        Ok(cookie) => {
                            REGISTRATION.store(cookie, Ordering::SeqCst);
                            let _ = registered.send(Ok(()));
                        }
                        Err(error) => {
                            let _ = registered
                                .send(Err(format!("toast activator registration failed: {error}")));
                            return;
                        }
                    }
                    // Hold the apartment open. The class object is reachable
                    // only while this thread keeps the MTA alive.
                    loop {
                        std::thread::park();
                    }
                })
                .map_err(|error| format!("toast activator thread: {error}"))?;
            ready
                .recv()
                .unwrap_or_else(|_| Err("toast activator thread stopped".to_string()))
        });
        *OUTCOME.lock().unwrap_or_else(|error| error.into_inner()) = Some(outcome);
    });

    match OUTCOME
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
    {
        Some(Ok(())) => Ok(()),
        Some(Err(message)) => Err(PlatformError::Platform(message)),
        None => Err(PlatformError::Platform(
            "toast activator registration did not complete".to_string(),
        )),
    }
}

/// Remove this product's CLSID registration. For uninstall.
pub fn unregister_toast_activator(aumid: &str) {
    let cookie = REGISTRATION.swap(0, Ordering::SeqCst);
    if cookie != 0 {
        let _ = unsafe { CoRevokeClassObject(cookie) };
    }
    let path = clsid_key_path(&clsid_for(aumid));
    let wide = wide(&path);
    let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide.as_ptr())) };
}

fn clsid_key_path(clsid: &GUID) -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{{{clsid:?}}}\\LocalServer32")
}

fn write_local_server(clsid: &GUID) -> Result<(), PlatformError> {
    let exe = super::app::launch_executable()
        .map_err(|error| PlatformError::Platform(format!("current exe: {error}")))?;
    let command = format!("\"{}\"", exe.display());
    let key_path = wide(&clsid_key_path(clsid));
    let value = wide(&command);
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|error| PlatformError::Platform(format!("toast activator CLSID key: {error}")))?;
        let bytes = std::slice::from_raw_parts(
            value.as_ptr().cast::<u8>(),
            std::mem::size_of_val(value.as_slice()),
        );
        let result = RegSetValueExW(key, None, None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
        result.ok().map_err(|error| {
            PlatformError::Platform(format!("toast activator LocalServer32: {error}"))
        })
    }
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[implement(INotificationActivationCallback)]
struct Activator;

impl INotificationActivationCallback_Impl for Activator_Impl {
    fn Activate(
        &self,
        _app_user_model_id: &PCWSTR,
        invoked_args: &PCWSTR,
        _data: *const NOTIFICATION_USER_INPUT_DATA,
        _count: u32,
    ) -> WinResult<()> {
        let args = unsafe { invoked_args.to_string() }.unwrap_or_default();
        super::notification::on_toast_activated(&args);
        Ok(())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<'_, IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> WinResult<()> {
        if object.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let callback: INotificationActivationCallback = Activator.into();
        unsafe { callback.query(&*iid, object).ok() }.map_err(|_| E_NOINTERFACE.into())
    }

    fn LockServer(&self, _lock: BOOL) -> WinResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_payload_round_trips_and_rejects_anything_else() {
        let payload = launch_payload("abc123");
        assert_eq!(token_from_launch(&payload), Some("abc123"));
        assert_eq!(token_from_launch("https://example.com/x"), None);
        assert_eq!(token_from_launch(""), None);
    }

    #[test]
    fn the_clsid_is_stable_per_aumid_and_differs_between_products() {
        assert_eq!(clsid_for("com.example.one"), clsid_for("com.example.one"));
        assert_ne!(clsid_for("com.example.one"), clsid_for("com.example.two"));
    }
}
