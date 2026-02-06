use super::register_host;
use crate::LxApp;
use crate::LxAppError;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::device::Device;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct MakePhoneCallParams {
    #[serde(rename = "phoneNumber")]
    phone_number: String,
}

fn make_phone_call_impl(lxapp: &LxApp, params: &MakePhoneCallParams) -> Result<(), LxAppError> {
    lxapp
        .runtime
        .make_phone_call(&params.phone_number)
        .map_err(|e| LxAppError::Runtime(format!("Failed to make phone call: {}", e)))
}

host_api!(MakePhoneCall, MakePhoneCallParams, (), |lxapp, params| {
    make_phone_call_impl(&lxapp, &params)
});

#[derive(Deserialize)]
struct OpenUrlOptions {
    #[serde(rename = "url")]
    url: String,
    #[serde(rename = "openIn")]
    _open_in: Option<String>,
}

fn open_url_impl(lxapp: &LxApp, options: &OpenUrlOptions) -> Result<(), LxAppError> {
    if options.url.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "openURL requires url".to_string(),
        ));
    }
    lxapp
        .runtime
        .launch_with_url(options.url.clone())
        .map_err(|e| LxAppError::Runtime(format!("openURL failed: {}", e)))?;
    Ok(())
}

host_api!(OpenURL, OpenUrlOptions, (), |lxapp, options| {
    open_url_impl(&lxapp, &options)
});

pub(crate) fn register_all() {
    register_host("makePhoneCall", Arc::new(MakePhoneCall));
    register_host("openURL", Arc::new(OpenURL));
}
