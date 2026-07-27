use lxapp::{LxApp, is_public_network_address};
use rong_rt::http::{HttpError, NetworkAccessGuard, Uri};
use std::future::Future;
use std::net::{IpAddr, ToSocketAddrs};
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

        if !self.lxapp.is_domain_allowed(host) {
            return Err(HttpError::access_denied(format!(
                "Network access denied: domain '{host}' is not allowed by lxapp security policy"
            )));
        }
        ensure_public_target(host, uri.port_u16().unwrap_or(0))
    }
}

fn ensure_public_target(host: &str, port: u16) -> Result<(), HttpError> {
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let addresses: Vec<IpAddr> = if let Ok(address) = unbracketed.parse() {
        vec![address]
    } else {
        (unbracketed, port)
            .to_socket_addrs()
            .map_err(|error| {
                HttpError::access_denied(format!(
                    "Network access denied: failed to resolve domain '{host}': {error}"
                ))
            })?
            .map(|address| address.ip())
            .collect()
    };
    if addresses.is_empty() {
        return Err(HttpError::access_denied(format!(
            "Network access denied: domain '{host}' resolved to no addresses"
        )));
    }
    if let Some(address) = addresses
        .into_iter()
        .find(|address| !is_public_network_address(*address))
    {
        return Err(HttpError::access_denied(format!(
            "Network access denied: domain '{host}' resolves to non-public address {address}"
        )));
    }
    Ok(())
}

pub(super) async fn scope_lxapp_network_access<F, T>(lxapp: Arc<LxApp>, future: F) -> T
where
    F: Future<Output = T>,
{
    rong_rt::http::scope_network_access_guard(Arc::new(LxAppNetworkAccessGuard { lxapp }), future)
        .await
}

#[cfg(test)]
mod tests {
    use super::ensure_public_target;

    #[test]
    fn rejects_private_and_loopback_literal_targets() {
        assert!(ensure_public_target("127.0.0.1", 80).is_err());
        assert!(ensure_public_target("169.254.169.254", 80).is_err());
        assert!(ensure_public_target("[::1]", 80).is_err());
    }

    #[test]
    fn accepts_public_literal_targets() {
        assert!(ensure_public_target("1.1.1.1", 443).is_ok());
        assert!(ensure_public_target("[2606:4700:4700::1111]", 443).is_ok());
    }
}
