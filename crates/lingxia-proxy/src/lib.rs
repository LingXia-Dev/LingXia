mod error;
mod router;
mod server;
mod upstream;

#[cfg(feature = "rule-list-routing")]
pub mod rule_list;

#[cfg(feature = "capture")]
pub mod capture;
#[cfg(feature = "capture")]
pub(crate) mod mitm;

pub use error::ProxyError;
pub use router::{
    FixedRouter, Profile, ProfileRouter, ProxyRouter, RouteDecision, Socks5Credentials,
    UpstreamConfig,
};
pub use server::LocalProxy;

#[cfg(feature = "rule-list-routing")]
pub use rule_list::RuleListRouter;

#[cfg(feature = "capture")]
pub use capture::{
    CapturedBody, CapturedRequest, CapturedResponse, CapturedSession, SessionTiming,
};
#[cfg(feature = "capture")]
pub use mitm::CaConfig;
