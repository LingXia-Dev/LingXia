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
        ensure_public_target(
            host,
            uri.port_u16().unwrap_or(0),
            is_dev_fixture_target(host, lxapp::is_dev_session()),
        )
    }
}

/// A dev-session fixture is reached by IP literal, where there is no DNS step
/// for anyone to rebind. Relaxing on that alone keeps the guard doing its real
/// job: a *domain* the lxapp trusts that resolves into the LAN stays refused,
/// in a dev session as much as in a release build.
///
/// Testing `is_domain_allowed` here instead would be tautological — the caller
/// has already proven it — and would switch the guard off for every trusted
/// domain whenever a dev session is attached.
fn is_dev_fixture_target(host: &str, dev_session: bool) -> bool {
    if !dev_session {
        return false;
    }
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
        .parse::<IpAddr>()
        .is_ok()
}

/// Rejects a host that resolves to a non-public address, so a trusted public
/// domain cannot be rebound onto the user's own network. `allow_private` is the
/// dev-session fixture case, where the target is a literal address.
fn ensure_public_target(host: &str, port: u16, allow_private: bool) -> Result<(), HttpError> {
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
    if allow_private {
        return Ok(());
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
    use super::{ensure_public_target, is_dev_fixture_target};

    #[test]
    fn rejects_private_and_loopback_literal_targets() {
        assert!(ensure_public_target("127.0.0.1", 80, false).is_err());
        assert!(ensure_public_target("169.254.169.254", 80, false).is_err());
        assert!(ensure_public_target("[::1]", 80, false).is_err());
    }

    #[test]
    fn accepts_public_literal_targets() {
        assert!(ensure_public_target("1.1.1.1", 443, false).is_ok());
        assert!(ensure_public_target("[2606:4700:4700::1111]", 443, false).is_ok());
    }

    #[test]
    fn only_a_literal_address_counts_as_a_dev_fixture() {
        // The fixture case: an address, so no name to rebind.
        assert!(is_dev_fixture_target("127.0.0.1", true));
        assert!(is_dev_fixture_target("[::1]", true));
        assert!(is_dev_fixture_target("192.168.1.10", true));
        // A trusted domain keeps the guard even in a dev session, or a name
        // that resolves into the LAN would reach it.
        assert!(!is_dev_fixture_target("cn.bing.com", true));
        assert!(!is_dev_fixture_target("localhost", true));
        // And a release build never relaxes at all.
        assert!(!is_dev_fixture_target("127.0.0.1", false));
    }

    #[test]
    fn a_permitted_dev_fixture_passes_the_rebinding_guard() {
        assert!(ensure_public_target("127.0.0.1", 8117, true).is_ok());
        // The metadata endpoint is reachable too, which is why the caller must
        // only pass true for a host the lxapp's own policy already trusts.
        assert!(ensure_public_target("169.254.169.254", 80, true).is_ok());
    }
}
