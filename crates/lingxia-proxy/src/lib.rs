mod error;
mod router;
mod server;
mod upstream;

#[cfg(feature = "gfwlist")]
pub mod gfwlist;

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

#[cfg(feature = "gfwlist")]
pub use gfwlist::GfwlistRouter;

#[cfg(feature = "capture")]
pub use capture::{
    CapturedBody, CapturedRequest, CapturedResponse, CapturedSession, SessionTiming,
};
#[cfg(feature = "capture")]
pub use mitm::CaConfig;
