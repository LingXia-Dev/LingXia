use crate::LxApp;
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::device::Device;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct MakePhoneCallParams {
    #[serde(rename = "phoneNumber")]
    phone_number: String,
}

fn make_phone_call_impl(
    lxapp: &LxApp,
    params: &MakePhoneCallParams,
) -> Result<(), lingxia_error::Error> {
    lxapp.runtime.make_phone_call(&params.phone_number)
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

fn open_url_impl(lxapp: &LxApp, options: &OpenUrlOptions) -> Result<(), lingxia_error::Error> {
    if options.url.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg("openUrl requires url"));
    }

    let target = OpenUrlTarget::parse(options.target.as_deref());
    lxapp.runtime.open_url(OpenUrlRequest {
        owner_appid: lxapp.appid.clone(),
        owner_session_id: lxapp.session_id(),
        url: options.url.clone(),
        target,
        want_tab_id: false,
    })?;
    Ok(())
}

host_api!(OpenUrl, OpenUrlOptions, (), |lxapp, options| {
    open_url_impl(&lxapp, &options)
});

pub(crate) fn register_all() {
    register_host_module!("device", crate::host::RouteAudience::AppSessionOnly, {
        "makePhoneCall" => Arc::new(MakePhoneCall),
        "openUrl" => Arc::new(OpenUrl)
    });
}
