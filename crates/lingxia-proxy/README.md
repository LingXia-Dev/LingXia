# lingxia-proxy

A local proxy runtime used by LingXia browser shells.

It provides:

- A local HTTP proxy listener bound on `127.0.0.1:*`
- Runtime-swappable routing between `Direct` and upstream `SOCKS5`
- Optional rule-list based proxy routing with cached rule updates
- Optional HTTPS MITM capture for development/debugging

## Scope

This crate is a low-level runtime component. It does not define product UI,
settings pages, or JS host APIs. In the current workspace those are owned by
`lingxia-browser-shell`.

The main public types exported by this crate are:

- `LocalProxy`
- `ProxyRouter`
- `FixedRouter`
- `ProfileRouter`
- `Profile`
- `RouteDecision`
- `UpstreamConfig`
- `Socks5Credentials`
- `RuleListRouter` with feature `rule-list-routing`
- `CaConfig` and capture session types with feature `capture`

## Quick Start

```rust
use std::sync::Arc;
use lingxia_proxy::{FixedRouter, LocalProxy, UpstreamConfig};

let router = Arc::new(FixedRouter(UpstreamConfig::Direct));
let proxy = LocalProxy::bind("127.0.0.1:0", router).await?;
let addr = proxy.local_addr();

let proxy_task = proxy.clone();
tokio::spawn(async move {
    proxy_task.run().await;
});
```

`LocalProxy` accepts:

- `CONNECT host:port` tunnel requests
- Standard HTTP proxy requests for plain `http://` traffic

## Routing

### Fixed Upstream

```rust
use std::sync::Arc;
use lingxia_proxy::{FixedRouter, Socks5Credentials, UpstreamConfig};

let router = Arc::new(FixedRouter(UpstreamConfig::Socks5 {
    host: "127.0.0.1".into(),
    port: 1080,
    credentials: Some(Socks5Credentials {
        username: "user".into(),
        password: "pass".into(),
    }),
}));
proxy.set_router(router);
```

### Named Profiles

```rust
use std::sync::Arc;
use lingxia_proxy::{Profile, ProfileRouter, UpstreamConfig};

let mut router = ProfileRouter::new(vec![
    Profile {
        name: "direct".into(),
        upstream: UpstreamConfig::Direct,
    },
    Profile {
        name: "proxy".into(),
        upstream: UpstreamConfig::Socks5 {
            host: "127.0.0.1".into(),
            port: 1080,
            credentials: None,
        },
    },
]);
router.activate("proxy");
proxy.set_router(Arc::new(router));
```

### Custom Router

```rust
use lingxia_proxy::{ProxyError, ProxyRouter, RouteDecision, UpstreamConfig};

struct BlockExample;

impl ProxyRouter for BlockExample {
    fn route(&self, host: &str, _port: u16) -> Result<RouteDecision, ProxyError> {
        if host.ends_with(".example.invalid") {
            return Ok(RouteDecision::Block);
        }
        Ok(RouteDecision::Upstream(UpstreamConfig::Direct))
    }
}
```

## Rule-List Routing

Enable feature `rule-list-routing` to use:

- `RuleListRouter`
- `fetch_encoded_from_url(...)`
- `validate_source_url(...)`

This is intended for shells that want an "Auto Switch" style mode backed by a
downloaded rule list plus their own higher-level settings/runtime logic. The
current implementation supports gfwlist-compatible sources, but the Cargo
feature is named after the product capability rather than a specific source.

## HTTPS MITM Capture

Enable feature `capture` to use:

- `CaConfig`
- `session_receiver()`
- `CapturedSession` and related capture types

Important notes:

- This is for development/debugging use.
- The caller must provide and install a CA certificate.
- Upstream HTTPS connections are validated against the system/native root store.
- If upstream TLS validation fails, interception fails instead of silently
  trusting the upstream certificate.

## Feature Flags

- `rule-list-routing`
  Adds rule-list routing and HTTPS download support for rule list refresh.
- `capture`
  Adds MITM interception and structured HTTP capture types.
