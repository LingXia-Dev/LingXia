use super::registry;
use super::security::{NetworkSecurity, normalize_security_privilege_id, normalize_trusted_domain};
use crate::provider::{LxAppChannel, LxAppPermissions, lxapp_registry_provider};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

const PERMISSION_TIMEOUT: Duration = Duration::from_secs(5);

enum PrivilegePolicy {
    Unrestricted,
    Allowlist(BTreeSet<String>),
}

/// What applies to this instance. Default is deny, which only a pending or
/// unintelligible answer ever resolves to.
#[derive(Default)]
struct HostGrant {
    network: NetworkSecurity,
    privileges: PrivilegePolicy,
}

impl Default for PrivilegePolicy {
    fn default() -> Self {
        Self::Allowlist(BTreeSet::new())
    }
}

impl HostGrant {
    /// Public network and every privilege class.
    fn unrestricted() -> Self {
        let mut network = NetworkSecurity::new();
        network.set_domains(&["*".to_string()]);
        Self {
            network,
            privileges: PrivilegePolicy::Unrestricted,
        }
    }

    fn allows_privilege(&self, privilege: &str) -> bool {
        match &self.privileges {
            PrivilegePolicy::Unrestricted => true,
            PrivilegePolicy::Allowlist(granted) => {
                granted.contains("*") || granted.contains(privilege)
            }
        }
    }
}

/// One decision per instance, taken from the app's registry record.
///
/// Nothing restricts an app except a grant the registry actually returned: no
/// provider, no record, a registry that cannot be reached, and a record with no
/// grant all leave it unrestricted. A pending decision denies until it lands.
pub(super) struct HostPermissions {
    receiver: watch::Receiver<Option<Arc<HostGrant>>>,
}

impl Default for HostPermissions {
    fn default() -> Self {
        Self::resolved(HostGrant::default())
    }
}

impl HostPermissions {
    fn resolved(grant: HostGrant) -> Self {
        let (_, receiver) = watch::channel(Some(Arc::new(grant)));
        Self { receiver }
    }

    /// Home, and every app the registry does not constrain, run unrestricted.
    ///
    /// The pre-open status gate has usually just refreshed this record, so the
    /// grant is on hand and the instance starts already decided — no request of
    /// its own, and no window where a page paints before the answer.
    pub(super) fn start(appid: &str, channel: LxAppChannel, trusted_home: bool) -> Self {
        if trusted_home || lxapp_registry_provider().is_none() {
            return Self::resolved(HostGrant::unrestricted());
        }
        if let Some(cached) = registry::cached_grant(appid, channel) {
            return Self::resolved(grant_from(cached));
        }
        Self::query(appid, channel)
    }

    pub(super) fn query(appid: &str, channel: LxAppChannel) -> Self {
        let (sender, receiver) = watch::channel(None);
        let appid = appid.to_string();
        std::mem::drop(crate::executor::spawn(async move {
            let grant = tokio::select! {
                _ = sender.closed() => return,
                result = resolve(&appid, channel, PERMISSION_TIMEOUT) => result,
            };
            sender.send_replace(Some(Arc::new(grant)));
        }));
        Self { receiver }
    }

    pub(super) fn is_ready(&self) -> bool {
        self.receiver.borrow().is_some()
    }

    pub(super) async fn wait_ready(&self) {
        let mut receiver = self.receiver.clone();
        // A provider panic closes the channel; the unresolved state still denies.
        let _ = receiver.wait_for(Option::is_some).await;
    }

    pub(super) fn domains(&self) -> Vec<String> {
        self.receiver
            .borrow()
            .as_ref()
            .map(|grant| grant.network.domains())
            .unwrap_or_default()
    }

    pub(super) fn is_domain_allowed(&self, domain: &str, dev_session: bool) -> bool {
        self.receiver
            .borrow()
            .as_ref()
            .is_some_and(|grant| grant.network.is_domain_allowed_in(domain, dev_session))
    }

    pub(super) fn allows_privilege(&self, privilege: &str) -> bool {
        self.receiver
            .borrow()
            .as_ref()
            .is_some_and(|grant| grant.allows_privilege(privilege))
    }
}

/// Resolves a snapshot a test deliberately left pending.
#[cfg(test)]
pub(crate) struct DeferredGrant(watch::Sender<Option<Arc<HostGrant>>>);

#[cfg(test)]
impl DeferredGrant {
    /// Land the decision, as the registry lookup would.
    pub(crate) fn resolve(&self, permissions: Option<LxAppPermissions>) {
        self.0.send_replace(Some(Arc::new(grant_from(permissions))));
    }
}

#[cfg(test)]
impl HostPermissions {
    /// A snapshot that has not landed, plus the handle that lands it.
    pub(crate) fn deferred() -> (Self, DeferredGrant) {
        let (sender, receiver) = watch::channel(None);
        (Self { receiver }, DeferredGrant(sender))
    }
}

/// A registry that answers too slowly is no more evidence than one that cannot
/// be reached: the app keeps its standing grant, or none.
async fn resolve(appid: &str, channel: LxAppChannel, timeout: Duration) -> HostGrant {
    let permissions =
        match tokio::time::timeout(timeout, registry::resolve_grant(appid, channel)).await {
            Ok(permissions) => permissions,
            Err(_) => {
                crate::warn!("Registry grant lookup timed out for {}", appid).with_appid(appid);
                registry::standing_grant(appid, channel)
            }
        };
    grant_from(permissions)
}

/// A grant the registry did return but we cannot read is the one answer that
/// denies: the server asserted a policy, and guessing at it would widen the app
/// past what it asked for.
fn grant_from(permissions: Option<LxAppPermissions>) -> HostGrant {
    let Some(permissions) = permissions else {
        return HostGrant::unrestricted();
    };
    match approve(permissions) {
        Ok(grant) => grant,
        Err(reason) => {
            crate::warn!("Permissions denied: invalid lxapp grant ({})", reason);
            HostGrant::default()
        }
    }
}

fn approve(permissions: LxAppPermissions) -> Result<HostGrant, &'static str> {
    let mut network = NetworkSecurity::new();
    match permissions.network {
        None => network.set_domains(&["*".to_string()]),
        Some(granted) => {
            let granted_domains = granted
                .trusted_domains
                .iter()
                .map(|domain| normalize_trusted_domain(domain).ok_or("invalid host"))
                .collect::<Result<BTreeSet<_>, _>>()?;
            if granted_domains.len() > 1 && granted_domains.contains("*") {
                return Err("wildcard cannot be combined with other hosts");
            }
            network.set_domains(&granted_domains.into_iter().collect::<Vec<_>>());
        }
    }

    let privileges = match permissions.privileges {
        None => PrivilegePolicy::Unrestricted,
        Some(granted) => PrivilegePolicy::Allowlist(
            granted
                .granted
                .iter()
                .map(|privilege| {
                    if privilege == "*" {
                        Ok("*".to_string())
                    } else {
                        normalize_security_privilege_id(privilege).ok_or("invalid privilege id")
                    }
                })
                .collect::<Result<BTreeSet<_>, _>>()?,
        ),
    };

    Ok(HostGrant {
        network,
        privileges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(domains: &[&str]) -> LxAppPermissions {
        LxAppPermissions::network(domains.iter().copied())
    }

    #[test]
    fn a_network_grant_is_the_allowlist() {
        let full = approve(LxAppPermissions::all()).unwrap();
        assert!(full.network.is_domain_allowed_in("api.example.com", false));
        assert!(full.allows_privilege("downloads"));

        let covering = approve(grant(&["*.example.com"])).unwrap();
        assert!(
            covering
                .network
                .is_domain_allowed_in("api.example.com", false)
        );
        assert!(!covering.network.is_domain_allowed_in("other.org", false));
        assert!(covering.allows_privilege("downloads"));

        let named = approve(grant(&["api.example.com"])).unwrap();
        assert!(named.network.is_domain_allowed_in("api.example.com", false));
        assert!(
            !named
                .network
                .is_domain_allowed_in("other.example.com", false)
        );
        assert!(named.allows_privilege("process"));
    }

    #[test]
    fn an_unconstrained_half_stays_unrestricted() {
        let network_only = approve(LxAppPermissions::network(["api.example.com"])).unwrap();
        assert!(
            network_only
                .network
                .is_domain_allowed_in("api.example.com", false)
        );
        assert!(network_only.allows_privilege("downloads"));
        assert!(network_only.allows_privilege("process"));

        let privileges_only = approve(LxAppPermissions::privileges(["downloads"])).unwrap();
        assert!(
            privileges_only
                .network
                .is_domain_allowed_in("api.example.com", false)
        );
        assert!(privileges_only.allows_privilege("downloads"));
        assert!(!privileges_only.allows_privilege("process"));

        let none = approve(
            LxAppPermissions::network(std::iter::empty::<String>())
                .with_privileges(std::iter::empty::<String>()),
        )
        .unwrap();
        assert!(!none.network.is_domain_allowed_in("api.example.com", false));
        assert!(!none.allows_privilege("downloads"));
    }

    #[test]
    fn a_privilege_allowlist_is_the_grant() {
        let extra = approve(LxAppPermissions::privileges(["downloads"])).unwrap();
        assert!(extra.allows_privilege("downloads"));
        assert!(!extra.allows_privilege("automation"));
        let star = approve(LxAppPermissions::privileges(["*"])).unwrap();
        assert!(star.allows_privilege("downloads"));
        assert!(star.allows_privilege("automation"));
    }

    #[test]
    fn a_malformed_grant_is_refused_whole() {
        assert!(approve(grant(&["https://api.example.com"])).is_err());
        assert!(approve(grant(&["*", "api.example.com"])).is_err());
        assert!(approve(LxAppPermissions::privileges(["Downloads!"])).is_err());
    }

    #[test]
    fn no_grant_allows_and_an_unreadable_one_denies() {
        let unconstrained = grant_from(None);
        assert!(
            unconstrained
                .network
                .is_domain_allowed_in("api.example.com", false)
        );
        assert!(unconstrained.allows_privilege("automation"));
        // Loopback is not "public network" and stays out of a dev session.
        assert!(
            !unconstrained
                .network
                .is_domain_allowed_in("127.0.0.1", false)
        );

        let unreadable = grant_from(Some(grant(&["https://api.example.com"])));
        assert!(
            !unreadable
                .network
                .is_domain_allowed_in("api.example.com", false)
        );
        assert!(!unreadable.allows_privilege("downloads"));
    }

    #[test]
    fn home_and_a_registry_free_host_are_unrestricted() {
        for permissions in [
            HostPermissions::start("host-selected-home", LxAppChannel::Release, true),
            HostPermissions::start("guest", LxAppChannel::Release, false),
        ] {
            assert!(permissions.is_ready());
            assert!(permissions.is_domain_allowed("api.example.com", false));
            assert!(permissions.allows_privilege("downloads"));
            assert!(permissions.allows_privilege("automation"));
        }
    }

    #[test]
    fn pending_and_abandoned_snapshots_deny() {
        let (sender, receiver) = watch::channel(None);
        let permissions = HostPermissions { receiver };
        assert!(!permissions.is_ready());
        assert!(!permissions.is_domain_allowed("api.example.com", false));
        assert!(!permissions.allows_privilege("downloads"));
        assert!(permissions.domains().is_empty());
        drop(sender);
        block_on(permissions.wait_ready());
        assert!(!permissions.is_ready());
        assert!(!permissions.is_domain_allowed("api.example.com", false));
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }
}
