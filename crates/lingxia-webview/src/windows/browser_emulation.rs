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

static CONFIGURED_PROFILE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub(crate) fn configured_profile() -> Option<WindowsBrowserEmulationProfile> {
    match CONFIGURED_PROFILE.load(std::sync::atomic::Ordering::Acquire) {
        1 => Some(WindowsBrowserEmulationProfile::Desktop),
        2 => Some(WindowsBrowserEmulationProfile::Phone),
        3 => Some(WindowsBrowserEmulationProfile::Tablet),
        _ => None,
    }
}

/// Configures the profile inherited by WebViews created afterwards.
pub fn set_windows_browser_emulation_profile_for_new_webviews(
    profile: WindowsBrowserEmulationProfile,
) {
    CONFIGURED_PROFILE.store(profile as u8 + 1, std::sync::atomic::Ordering::Release);
}

pub(crate) fn apply_profile(
    webview: &ICoreWebView2,
    default_user_agent: &str,
    default_user_agent_metadata: &serde_json::Value,
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

    let mut metadata = default_user_agent_metadata.clone();
    let Some(metadata) = metadata.as_object_mut() else {
        let _ = resp.send(Err(WebViewError::WebView(
            "WebView2 supplied invalid default User-Agent Client Hints".to_string(),
        )));
        return;
    };
    let platform = if profile == WindowsBrowserEmulationProfile::Desktop {
        metadata
            .get("platform")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Windows")
            .to_string()
    } else {
        metadata.insert("platform".into(), "Android".into());
        metadata.insert("platformVersion".into(), "10.0.0".into());
        metadata.insert("architecture".into(), "".into());
        metadata.insert("model".into(), "".into());
        metadata.insert("bitness".into(), "".into());
        metadata.insert("wow64".into(), false.into());
        metadata.insert(
            "formFactors".into(),
            serde_json::json!([match profile {
                WindowsBrowserEmulationProfile::Phone => "Mobile",
                WindowsBrowserEmulationProfile::Tablet => "Tablet",
                WindowsBrowserEmulationProfile::Desktop => unreachable!(),
            }]),
        );
        "Android".to_string()
    };
    metadata.insert(
        "mobile".into(),
        (profile == WindowsBrowserEmulationProfile::Phone).into(),
    );
    let params = serde_json::json!({
        "userAgent": user_agent,
        "platform": platform,
        "userAgentMetadata": metadata,
    })
    .to_string();
    start_call_devtools_protocol(webview, "Emulation.setUserAgentOverride", &params, resp);
}

pub(crate) fn start_capture_default_metadata(
    webview: &ICoreWebView2,
    resp: Sender<StdResult<String>>,
) {
    let expression = r#"(async () => {
        const data = navigator.userAgentData;
        if (!data) return null;
        return await data.getHighEntropyValues([
            'architecture', 'bitness', 'formFactors', 'fullVersionList',
            'model', 'platformVersion', 'wow64'
        ]);
    })()"#;
    let params = serde_json::json!({
        "expression": expression,
        "awaitPromise": true,
        "returnByValue": true,
    })
    .to_string();
    start_call_devtools_protocol(webview, "Runtime.evaluate", &params, resp);
}

pub(crate) fn decode_default_metadata(response: &str) -> StdResult<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(response)
        .ok()
        .and_then(|value| value.get("result")?.get("value").cloned())
        .filter(serde_json::Value::is_object)
        .ok_or_else(|| {
            WebViewError::WebView(
                "WebView2 did not expose default User-Agent Client Hints".to_string(),
            )
        })
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

    #[test]
    fn decodes_runtime_evaluate_metadata() {
        let metadata = decode_default_metadata(
            r#"{"result":{"type":"object","value":{"brands":[],"mobile":false,"platform":"Windows"}}}"#,
        )
        .expect("metadata");

        assert_eq!(metadata["platform"], "Windows");
        assert_eq!(metadata["mobile"], false);
    }
}
