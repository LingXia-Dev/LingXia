use super::register_host;
use crate::LxApp;
use crate::LxAppError;
use lingxia_platform::PlatformError;
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::device::Device;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct MakePhoneCallParams {
    #[serde(rename = "phoneNumber")]
    phone_number: String,
}

fn map_platform_error(api: &str, error: PlatformError) -> LxAppError {
    let (code, message) = match error {
        PlatformError::NotSupported(message) => ("E_NOT_SUPPORTED", message),
        PlatformError::InvalidParameter(message) => ("E_INVALID_PARAMETER", message),
        PlatformError::AssetNotFound(message) => ("E_NOT_FOUND", message),
        PlatformError::Platform(message) => ("E_PLATFORM", message),
        PlatformError::BusinessError(code) => {
            return LxAppError::RongJSHost {
                code: code.to_string(),
                message: format!("{api} failed"),
                data: Some(serde_json::json!({ "bizCode": code })),
            };
        }
        PlatformError::CallbackDropped => ("E_CALLBACK_DROPPED", "Callback dropped".to_string()),
    };

    LxAppError::RongJSHost {
        code: code.to_string(),
        message,
        data: None,
    }
}

fn make_phone_call_impl(lxapp: &LxApp, params: &MakePhoneCallParams) -> Result<(), LxAppError> {
    lxapp
        .runtime
        .make_phone_call(&params.phone_number)
        .map_err(|e| map_platform_error("makePhoneCall", e))
}

host_api!(MakePhoneCall, MakePhoneCallParams, (), |lxapp, params| {
    make_phone_call_impl(&lxapp, &params)
});

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenUrlOptions {
    #[serde(rename = "url")]
    url: String,
    #[serde(rename = "target")]
    target: Option<String>,
}

fn open_url_impl(lxapp: &LxApp, options: &OpenUrlOptions) -> Result<(), LxAppError> {
    if options.url.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "openURL requires url".to_string(),
        ));
    }

    let target = OpenUrlTarget::parse(options.target.as_deref());
    lxapp
        .runtime
        .open_url(OpenUrlRequest {
            owner_appid: lxapp.appid.clone(),
            owner_session_id: lxapp.session_id(),
            url: options.url.clone(),
            target,
        })
        .map_err(|e| map_platform_error("openURL", e))?;
    Ok(())
}

host_api!(OpenURL, OpenUrlOptions, (), |lxapp, options| {
    open_url_impl(&lxapp, &options)
});

pub(crate) fn register_all() {
    register_host("makePhoneCall", Arc::new(MakePhoneCall));
    register_host("openURL", Arc::new(OpenURL));
}
