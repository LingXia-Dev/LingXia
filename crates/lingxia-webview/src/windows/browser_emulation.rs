//! Browser identity emulation used by desktop device-preview hosts.

use super::*;

/// Browser form factor exposed by a Windows WebView2 preview.
///
/// This intentionally describes browser behavior, not a branded device. A
/// Chromium WebView2 cannot faithfully impersonate iOS Safari, so mobile
/// profiles retain the installed WebView2 engine versions and present a
/// generic Android phone or tablet identity instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsBrowserEmulationProfile {
    Desktop,
    Phone,
    Tablet,
}

static CONFIGURED_PROFILE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(WindowsBrowserEmulationProfile::Desktop as u8);

pub(crate) fn configured_profile() -> WindowsBrowserEmulationProfile {
    match CONFIGURED_PROFILE.load(std::sync::atomic::Ordering::Acquire) {
        value if value == WindowsBrowserEmulationProfile::Phone as u8 => {
            WindowsBrowserEmulationProfile::Phone
        }
        value if value == WindowsBrowserEmulationProfile::Tablet as u8 => {
            WindowsBrowserEmulationProfile::Tablet
        }
        _ => WindowsBrowserEmulationProfile::Desktop,
    }
}

/// Configures the profile inherited by WebViews created afterwards.
pub fn set_windows_browser_emulation_profile_for_new_webviews(
    profile: WindowsBrowserEmulationProfile,
) {
    CONFIGURED_PROFILE.store(profile as u8, std::sync::atomic::Ordering::Release);
}

pub(crate) fn apply_profile(
    webview: &ICoreWebView2,
    default_user_agent: &str,
    profile: WindowsBrowserEmulationProfile,
    resp: Sender<StdResult<String>>,
) {
    let user_agent = match effective_user_agent(default_user_agent, profile) {
        Ok(user_agent) => user_agent,
        Err(err) => {
            let _ = resp.send(Err(err));
            return;
        }
    };
    if let Err(err) = set_user_agent_override(webview, &user_agent) {
        let _ = resp.send(Err(err));
        return;
    }

    let params = serde_json::json!({
        "userAgent": user_agent,
        "platform": match profile {
            WindowsBrowserEmulationProfile::Desktop => "Windows",
            WindowsBrowserEmulationProfile::Phone | WindowsBrowserEmulationProfile::Tablet => "Android",
        },
        "userAgentMetadata": {
            "platform": match profile {
                WindowsBrowserEmulationProfile::Desktop => "Windows",
                WindowsBrowserEmulationProfile::Phone | WindowsBrowserEmulationProfile::Tablet => "Android",
            },
            "platformVersion": "10.0.0",
            "architecture": match profile {
                WindowsBrowserEmulationProfile::Desktop => "x86",
                WindowsBrowserEmulationProfile::Phone | WindowsBrowserEmulationProfile::Tablet => "",
            },
            "model": "",
            "mobile": profile == WindowsBrowserEmulationProfile::Phone,
            "bitness": match profile {
                WindowsBrowserEmulationProfile::Desktop => "64",
                WindowsBrowserEmulationProfile::Phone | WindowsBrowserEmulationProfile::Tablet => "",
            }
        }
    })
    .to_string();
    start_call_devtools_protocol(webview, "Emulation.setUserAgentOverride", &params, resp);
}

fn effective_user_agent(
    default_user_agent: &str,
    profile: WindowsBrowserEmulationProfile,
) -> StdResult<String> {
    if profile == WindowsBrowserEmulationProfile::Desktop {
        return Ok(default_user_agent.to_string());
    }
    if !default_user_agent.contains(" Chrome/") || !default_user_agent.contains(" Safari/") {
        return Err(WebViewError::WebView(
            "WebView2 supplied an unsupported default user agent".to_string(),
        ));
    }

    let platform_start = default_user_agent.find('(').ok_or_else(|| {
        WebViewError::WebView("WebView2 default user agent has no platform token".to_string())
    })?;
    let platform_end = default_user_agent[platform_start..]
        .find(')')
        .map(|offset| platform_start + offset)
        .ok_or_else(|| {
            WebViewError::WebView("WebView2 default user agent has no platform token".to_string())
        })?;
    let mut user_agent = format!(
        "{}(Linux; Android 10; K){}",
        &default_user_agent[..platform_start],
        &default_user_agent[platform_end + 1..]
    )
    .replace(" Edg/", " EdgA/");
    if profile == WindowsBrowserEmulationProfile::Phone {
        user_agent = user_agent.replacen(" Safari/", " Mobile Safari/", 1);
    }
    Ok(user_agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0";

    #[test]
    fn phone_keeps_engine_versions_and_adds_mobile_tokens() {
        let user_agent = effective_user_agent(DEFAULT, WindowsBrowserEmulationProfile::Phone)
            .expect("phone user agent");

        assert_eq!(
            user_agent,
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Mobile Safari/537.36 EdgA/132.0.0.0"
        );
    }

    #[test]
    fn tablet_omits_the_mobile_token() {
        let user_agent = effective_user_agent(DEFAULT, WindowsBrowserEmulationProfile::Tablet)
            .expect("tablet user agent");

        assert!(user_agent.contains("(Linux; Android 10; K)"));
        assert!(!user_agent.contains(" Mobile "));
        assert!(user_agent.contains(" EdgA/132.0.0.0"));
    }

    #[test]
    fn desktop_restores_the_exact_engine_default() {
        assert_eq!(
            effective_user_agent(DEFAULT, WindowsBrowserEmulationProfile::Desktop)
                .expect("desktop user agent"),
            DEFAULT
        );
    }

    #[test]
    fn refuses_to_build_a_malformed_mobile_identity() {
        assert!(effective_user_agent("lingxia", WindowsBrowserEmulationProfile::Phone).is_err());
    }

    #[test]
    fn accepts_webview2_defaults_without_an_edge_brand_token() {
        let default = DEFAULT.replace(" Edg/132.0.0.0", "");
        let user_agent = effective_user_agent(&default, WindowsBrowserEmulationProfile::Phone)
            .expect("unbranded WebView2 user agent");

        assert!(user_agent.contains(" Chrome/132.0.0.0 Mobile Safari/537.36"));
        assert!(!user_agent.contains(" EdgA/"));
    }
}
