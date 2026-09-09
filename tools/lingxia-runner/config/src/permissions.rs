use lingxia_provider::{
    BoxFuture, LxAppPermissions, LxAppRegistryInfo, LxAppRegistryProvider, LxAppRegistryRequest,
    ProviderError,
};
use serde::Deserialize;
use std::collections::BTreeMap;

const PERMISSIONS_ENV: &str = "LINGXIA_RUNNER_LXAPP_PERMISSIONS";

/// An empty allowlist: denies the half it is given to.
const NOTHING: [String; 0] = [];

/// One app's development grant. Omitted `domains` or `privileges` leaves that
/// half unconstrained. An empty list denies that half.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerGrant {
    #[serde(default)]
    domains: Option<Vec<String>>,
    #[serde(default)]
    privileges: Option<Vec<String>>,
}

/// A registry standing in for the server, so a developer can reproduce a
/// production grant locally. It answers permissions and nothing else: names,
/// icons and status keep resolving from the package.
///
/// Only listed apps are constrained — an unlisted one is an app this registry
/// has no policy for, exactly as with a real server. Unset means no provider at
/// all. Never reads the project tree.
pub struct RunnerRegistryProvider {
    grants: Result<BTreeMap<String, RunnerGrant>, String>,
}

impl RunnerRegistryProvider {
    /// A JSON object mapping app ids to `{"domains": [...], "privileges": [...]}`.
    /// Unset leaves the unrestricted default, or an injected provider, in
    /// charge. Malformed
    /// input denies every app rather than quietly widening them: the developer
    /// asked for a policy, and the runtime treats a lookup that only *failed*
    /// as no answer at all.
    pub fn from_env() -> Option<Self> {
        let value = std::env::var_os(PERMISSIONS_ENV)?;
        Some(Self::parse(&value.to_string_lossy()))
    }

    fn parse(value: &str) -> Self {
        Self {
            grants: serde_json::from_str(value)
                .map_err(|err| format!("invalid {PERMISSIONS_ENV}: {err}")),
        }
    }
}

impl LxAppRegistryProvider for RunnerRegistryProvider {
    fn fetch_registry_info<'a>(
        &'a self,
        app: LxAppRegistryRequest<'a>,
    ) -> BoxFuture<'a, Result<Option<LxAppRegistryInfo>, ProviderError>> {
        Box::pin(async move {
            let permissions = match self.grants.as_ref() {
                // A denial has to travel as a grant. Reporting the parse error
                // would read as an unreachable registry, which does not
                // restrict anything — the opposite of what was asked for.
                Err(reason) => {
                    eprintln!("[runner] {reason}; denying every lxapp");
                    LxAppPermissions::network(NOTHING).with_privileges(NOTHING)
                }
                Ok(grants) => {
                    let Some(grant) = grants.get(app.appid) else {
                        return Ok(None);
                    };
                    let mut permissions = LxAppPermissions::all();
                    if let Some(domains) = grant.domains.clone() {
                        permissions = permissions.with_network(domains);
                    }
                    if let Some(privileges) = grant.privileges.clone() {
                        permissions = permissions.with_privileges(privileges);
                    }
                    permissions
                }
            };
            Ok(Some(LxAppRegistryInfo {
                permissions: Some(permissions),
                ..LxAppRegistryInfo::default()
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_are_explicit_per_app_and_malformed_input_fails_closed() {
        let provider = RunnerRegistryProvider::parse(
            r#"{"demo":{"domains":["api.example.com"],"privileges":["downloads"]},"open":{}}"#,
        );
        let grants = provider.grants.unwrap();
        assert_eq!(grants["demo"].domains, Some(vec!["api.example.com".into()]));
        assert_eq!(grants["demo"].privileges, Some(vec!["downloads".into()]));
        assert!(grants["open"].domains.is_none());
        assert!(grants["open"].privileges.is_none());
        assert!(!grants.contains_key("other"));

        for malformed in [
            r#"{"demo":"*"}"#,
            r#"{"demo":["api.example.com"]}"#,
            r#"{"demo":{"trustedDomains":["api.example.com"]}}"#,
        ] {
            assert!(
                RunnerRegistryProvider::parse(malformed).grants.is_err(),
                "{malformed}"
            );
        }
    }

    #[test]
    fn malformed_input_denies_every_app_instead_of_reading_as_no_answer() {
        let provider = RunnerRegistryProvider::parse(r#"{"demo":"*"}"#);
        for appid in ["demo", "never-listed"] {
            let permissions = futures_lite_block_on(provider.fetch_registry_info(request(appid)))
                .unwrap()
                .expect("a denial travels as a grant, not as an error")
                .permissions
                .expect("an explicit grant");
            assert_eq!(
                permissions.network.map(|network| network.trusted_domains),
                Some(Vec::new()),
                "{appid}"
            );
            assert_eq!(
                permissions.privileges.map(|privileges| privileges.granted),
                Some(Vec::new()),
                "{appid}"
            );
        }
    }

    #[test]
    fn an_unlisted_app_is_not_this_registrys_business() {
        let provider = RunnerRegistryProvider::parse(r#"{"demo":{"domains":[]}}"#);
        let unlisted = futures_lite_block_on(provider.fetch_registry_info(request("other")));
        assert!(unlisted.unwrap().is_none());

        let listed = futures_lite_block_on(provider.fetch_registry_info(request("demo")))
            .unwrap()
            .expect("listed app is constrained");
        assert_eq!(
            listed
                .permissions
                .and_then(|permissions| permissions.network)
                .map(|network| network.trusted_domains),
            Some(Vec::new())
        );
    }

    fn request(appid: &str) -> LxAppRegistryRequest<'_> {
        LxAppRegistryRequest::new(appid, lingxia_provider::LxAppChannel::Release)
    }

    /// This provider never awaits anything, so one poll is the whole answer —
    /// cheaper than pulling a runtime into a deliberately dependency-light crate.
    fn futures_lite_block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Waker};
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("the runner registry answers without awaiting"),
        }
    }
}
