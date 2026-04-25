use lxapp::LxApp;
use rong_rt::http::{HttpError, NetworkAccessGuard, Uri};
use std::future::Future;
use std::sync::Arc;

struct LxAppNetworkAccessGuard {
    lxapp: Arc<LxApp>,
}

impl NetworkAccessGuard for LxAppNetworkAccessGuard {
    fn check_access(&self, uri: &Uri) -> Result<(), HttpError> {
        let Some(host) = uri.host() else {
            return Err(HttpError::access_denied(
                "Network access denied: URL has no host",
            ));
        };

        if self.lxapp.is_domain_allowed(host) {
            Ok(())
        } else {
            Err(HttpError::access_denied(format!(
                "Network access denied: domain '{host}' is not allowed by lxapp security policy"
            )))
        }
    }
}

pub(super) async fn scope_lxapp_network_access<F, T>(lxapp: Arc<LxApp>, future: F) -> T
where
    F: Future<Output = T>,
{
    rong_rt::http::scope_network_access_guard(Arc::new(LxAppNetworkAccessGuard { lxapp }), future)
        .await
}
