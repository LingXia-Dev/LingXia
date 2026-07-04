//! WebView2 environment/controller creation and per-webview
//! operations (settings, scripts, history, capture).

use super::*;

mod operations;
mod scripts;
mod settings;

pub(crate) use operations::*;
pub(crate) use scripts::*;
pub use settings::set_windows_context_menu_refresh_provider;
pub(crate) use settings::*;

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
    let transient_user_data_dir = transient_user_data_dir(webtag, effective_options)?;
    let user_data_folder = transient_user_data_dir
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
    Ok((environment, transient_user_data_dir))
}

fn transient_user_data_dir(
    webtag: &WebTag,
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<Option<PathBuf>> {
    if effective_options.profile != SecurityProfile::StrictDefault {
        return Ok(None);
    }

    let mut hash = 0xcbf29ce484222325u64;
    for byte in webtag.key().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let base_dir = configured_webview_user_data_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("lingxia-webview"))
        .join("strict")
        .join(format!("{}-{hash:016x}", std::process::id()));
    let mut dir = base_dir.clone();
    if dir.exists() {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(err) => {
                log::warn!(
                    "strict WebView2 profile {dir:?} is still in use; creating a fresh profile: {err}"
                );
                dir = strict_fallback_user_data_dir(&base_dir);
            }
        }
    }
    std::fs::create_dir_all(&dir).map_err(|err| {
        WebViewError::WebView(format!(
            "failed to create strict WebView2 profile {dir:?}: {err}"
        ))
    })?;
    Ok(Some(dir))
}

fn strict_fallback_user_data_dir(base_dir: &std::path::Path) -> PathBuf {
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
