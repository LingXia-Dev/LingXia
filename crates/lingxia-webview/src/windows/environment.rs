//! WebView2 environment/controller creation and per-webview
//! operations (settings, scripts, history, capture).

use super::*;

mod operations;
mod scripts;
mod settings;

pub(crate) use operations::*;
pub(crate) use scripts::*;
pub use settings::set_windows_context_menu_refresh_provider;
pub(crate) use settings::{
    configure_context_menu, configure_controller, configure_settings, create_controller,
};

/// Custom schemes registered on every WebView2 environment.
///
/// All webviews share one user data folder, and WebView2 fails environment
/// creation with 0x8007139F when two environments over the same folder carry
/// different options, so registration must be identical everywhere and is
/// the fixed union of the schemes the runtime serves. Which schemes a given
/// webview actually handles is still decided per webview by its
/// `WebResourceRequested` filters (see `registered_request_schemes`).
const WEBVIEW2_CUSTOM_SCHEME_REGISTRATIONS: &[&str] = &["lingxia", "lx"];

pub(crate) fn create_environment(
    webtag: &WebTag,
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<(ICoreWebView2Environment, Option<PathBuf>)> {
    let options = CoreWebView2EnvironmentOptions::default();
    let custom_schemes: Vec<String> = WEBVIEW2_CUSTOM_SCHEME_REGISTRATIONS
        .iter()
        .map(|scheme| scheme.to_string())
        .collect();
    let ephemeral_user_data_dir = ephemeral_user_data_dir(webtag, effective_options)?;
    let user_data_folder = ephemeral_user_data_dir
        .clone()
        .or_else(configured_webview_user_data_dir)
        .map(|path| {
            let _ = std::fs::create_dir_all(&path);
            path.to_string_lossy().to_string()
        });

    unsafe {
        let registrations = custom_schemes
            .into_iter()
            .map(|scheme| {
                let registration = CoreWebView2CustomSchemeRegistration::new(scheme);
                registration.set_has_authority_component(true);
                registration.set_treat_as_secure(true);
                Some(registration.into())
            })
            .collect();
        options.set_scheme_registrations(registrations);
    }
    let options_iface: ICoreWebView2EnvironmentOptions = options.into();

    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let user_data_folder = user_data_folder
                .as_ref()
                .map(|path| CoTaskMemPWSTR::from(path.as_str()));
            let user_data_folder = user_data_folder
                .as_ref()
                .map(|path| *path.as_ref().as_pcwstr())
                .unwrap_or(PCWSTR::null());
            CreateCoreWebView2EnvironmentWithOptions(
                windows::core::PCWSTR::null(),
                user_data_folder,
                &options_iface,
                &handler,
            )
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, environment| {
            result?;
            tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    let environment = rx
        .recv()
        .map_err(|_| WebViewError::WebView("Environment callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Environment creation failed: {err}")))?;
    Ok((environment, ephemeral_user_data_dir))
}

fn ephemeral_user_data_dir(
    webtag: &WebTag,
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<Option<PathBuf>> {
    // Strict-profile webviews keep their existing per-creation isolation;
    // browser-profile webviews opt into it through the data mode (auth sheets
    // and future private tabs), while ordinary browser tabs stay persistent.
    if effective_options.profile != SecurityProfile::StrictDefault
        && effective_options.data_mode != WebViewDataMode::Ephemeral
    {
        return Ok(None);
    }

    let mut hash = 0xcbf29ce484222325u64;
    for byte in webtag.key().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let ephemeral_root = configured_webview_user_data_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("lingxia-webview"))
        .join("ephemeral");
    cleanup_orphaned_ephemeral_profiles_once(&ephemeral_root);
    let base_dir = ephemeral_root.join(format!("{}-{hash:016x}", std::process::id()));
    let mut dir = base_dir.clone();
    if dir.exists() {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(err) => {
                log::warn!(
                    "ephemeral WebView2 profile {dir:?} is still in use; creating a fresh profile: {err}"
                );
                dir = ephemeral_fallback_user_data_dir(&base_dir);
            }
        }
    }
    std::fs::create_dir_all(&dir).map_err(|err| {
        WebViewError::WebView(format!(
            "failed to create ephemeral WebView2 profile {dir:?}: {err}"
        ))
    })?;
    Ok(Some(dir))
}

fn cleanup_orphaned_ephemeral_profiles_once(root: &std::path::Path) {
    static CLEANED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    CLEANED.get_or_init(|| {
        let root = root.to_path_buf();
        if let Err(err) = std::thread::Builder::new()
            .name("lingxia-webview-orphan-cleanup".to_string())
            .spawn(move || cleanup_orphaned_ephemeral_profiles(&root))
        {
            log::warn!("failed to start orphaned WebView2 profile cleanup: {err}");
        }
    });
}

fn cleanup_orphaned_ephemeral_profiles(root: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let current_pid = std::process::id();
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Some(owner_pid) = ephemeral_profile_owner_pid(&entry.file_name()) else {
            continue;
        };
        if owner_pid == current_pid || windows_process_is_running(owner_pid) {
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => removed += 1,
            Err(err) => log::warn!(
                "failed to remove orphaned ephemeral WebView2 profile {:?}: {err}",
                entry.path()
            ),
        }
    }
    if removed > 0 {
        log::info!("removed {removed} orphaned ephemeral WebView2 profiles");
    }
}

fn ephemeral_profile_owner_pid(name: &std::ffi::OsStr) -> Option<u32> {
    let mut parts = name.to_str()?.split('-');
    let pid = parts.next()?.parse().ok().filter(|pid| *pid > 0)?;
    let hash = parts.next()?;
    if hash.len() != 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    if !parts.all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_hexdigit())) {
        return None;
    }
    Some(pid)
}

fn windows_process_is_running(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            // Access denied still means that a process owns the PID. Only the
            // invalid-parameter result proves that no such process exists.
            Err(err) => err.code() != ERROR_INVALID_PARAMETER.to_hresult(),
        }
    }
}

fn ephemeral_fallback_user_data_dir(base_dir: &std::path::Path) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let name = base_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile");
    base_dir.with_file_name(format!("{name}-{nonce:x}"))
}

pub(crate) fn registered_request_schemes(registered_schemes: &[String]) -> Vec<String> {
    let mut schemes = if registered_schemes.is_empty() {
        vec!["lx".to_string()]
    } else {
        registered_schemes.to_vec()
    };
    schemes.sort_unstable();
    schemes.dedup();
    schemes
}

pub(crate) fn webview2_custom_schemes(registered_schemes: &[String]) -> Vec<String> {
    registered_request_schemes(registered_schemes)
        .into_iter()
        .filter(|scheme| scheme != "http" && scheme != "https")
        .collect()
}

#[cfg(test)]
mod ephemeral_profile_tests {
    use super::{cleanup_orphaned_ephemeral_profiles, ephemeral_profile_owner_pid};
    use std::ffi::OsStr;

    #[test]
    fn parses_base_and_fallback_profile_owners() {
        assert_eq!(
            ephemeral_profile_owner_pid(OsStr::new("420-0123456789abcdef")),
            Some(420)
        );
        assert_eq!(
            ephemeral_profile_owner_pid(OsStr::new("420-0123456789abcdef-fedcba")),
            Some(420)
        );
    }

    #[test]
    fn ignores_directories_outside_the_managed_profile_shape() {
        for name in [
            "0-0123456789abcdef",
            "profile-0123456789abcdef",
            "420-short",
            "420-0123456789abcdeg",
            "420-0123456789abcdef-not-hex",
        ] {
            assert_eq!(ephemeral_profile_owner_pid(OsStr::new(name)), None);
        }
    }

    #[test]
    fn removes_exited_process_profiles_and_preserves_the_current_process() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "lingxia-webview-orphan-test-{}-{nonce:x}",
            std::process::id()
        ));
        let current = root.join(format!("{}-0123456789abcdef", std::process::id()));
        let orphan = root.join(format!("{}-fedcba9876543210", u32::MAX));
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&orphan).unwrap();

        cleanup_orphaned_ephemeral_profiles(&root);

        assert!(current.exists());
        assert!(!orphan.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
