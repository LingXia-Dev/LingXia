use crate::error::ProxyError;
use serde::{Deserialize, Serialize};

/// Where to send the tunnelled traffic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpstreamConfig {
    /// Connect directly to the target host.
    Direct,
    /// Route through an upstream SOCKS5 proxy.
    Socks5 {
        host: String,
        port: u16,
        /// Optional username/password credentials.
        credentials: Option<Socks5Credentials>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Socks5Credentials {
    pub username: String,
    pub password: String,
}

/// The router's verdict for a given target host.
#[derive(Debug, Clone)]
pub enum RouteDecision {
    Upstream(UpstreamConfig),
    /// Block the connection entirely.
    Block,
}

/// Pluggable routing policy.  Implementations must be `Send + Sync` so the
/// server can hold an `Arc<dyn ProxyRouter>`.
pub trait ProxyRouter: Send + Sync {
    /// Decide how to handle a CONNECT to `host:port`.
    fn route(&self, host: &str, port: u16) -> Result<RouteDecision, ProxyError>;
}

// ── Built-in routers ───────────────────────────────────────────────────────

/// Always routes to the same upstream (or direct).
pub struct FixedRouter(pub UpstreamConfig);

impl ProxyRouter for FixedRouter {
    fn route(&self, _host: &str, _port: u16) -> Result<RouteDecision, ProxyError> {
        Ok(RouteDecision::Upstream(self.0.clone()))
    }
}

/// A named routing profile, similar to a SwitchyOmega profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub upstream: UpstreamConfig,
}

/// Routes based on a list of named profiles, with one active at a time.
/// Falls back to `Direct` when no profile is active.
pub struct ProfileRouter {
    profiles: Vec<Profile>,
    active: Option<String>,
}

impl ProfileRouter {
    pub fn new(profiles: Vec<Profile>) -> Self {
        Self {
            profiles,
            active: None,
        }
    }

    pub fn activate(&mut self, name: &str) -> bool {
        let found = self.profiles.iter().any(|p| p.name == name);
        if found {
            self.active = Some(name.to_owned());
        }
        found
    }

    pub fn deactivate(&mut self) {
        self.active = None;
    }
}

impl ProxyRouter for ProfileRouter {
    fn route(&self, _host: &str, _port: u16) -> Result<RouteDecision, ProxyError> {
        let upstream = self
            .active
            .as_deref()
            .and_then(|name| self.profiles.iter().find(|p| p.name == name))
            .map(|p| p.upstream.clone())
            .unwrap_or(UpstreamConfig::Direct);
        Ok(RouteDecision::Upstream(upstream))
    }
}
