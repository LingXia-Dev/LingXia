//! Rule-list based proxy routing (requires `feature = "rule-list-routing"`).
//!
//! Parses the community gfwlist (base64 + Adblock Plus rule format) into a
//! domain-suffix HashSet.  Routing decisions are a single RwLock read + O(n)
//! suffix walk (n = number of dot-separated labels, typically ≤ 4).
//!
//! # Update flow
//!
//! `fetch_encoded` connects **directly** through the caller-supplied SOCKS5
//! upstream, bypassing the local proxy entirely.  This ensures
//! `raw.githubusercontent.com` is reachable even before any rules are loaded.

use crate::error::ProxyError;
use crate::router::{ProxyRouter, RouteDecision, UpstreamConfig};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

#[cfg(feature = "rule-list-routing")]
use {
    crate::upstream::connect_upstream,
    http::Uri,
    tokio::io::{AsyncReadExt, AsyncWriteExt},
};

/// Default rule-list source URL. The default source is gfwlist-compatible.
pub const DEFAULT_RULE_LIST_URL: &str =
    "https://raw.githubusercontent.com/gfwlist/gfwlist/master/gfwlist.txt";

#[cfg(feature = "rule-list-routing")]
fn parse_source_url(url: &str) -> Result<(String, u16, String), ProxyError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(ProxyError::RuleList("source URL must not be empty".into()));
    }

    let uri: Uri = trimmed
        .parse()
        .map_err(|e| ProxyError::RuleList(format!("invalid source URL: {e}")))?;
    if uri.scheme_str() != Some("https") {
        return Err(ProxyError::RuleList(
            "source URL must use https".to_string(),
        ));
    }

    let host = uri
        .host()
        .ok_or_else(|| ProxyError::RuleList("source URL is missing host".into()))?
        .to_string();
    let port = uri.port_u16().unwrap_or(443);
    let path = uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    Ok((host, port, path))
}

#[cfg(feature = "rule-list-routing")]
pub fn validate_source_url(url: &str) -> Result<(), ProxyError> {
    let _ = parse_source_url(url)?;
    Ok(())
}

// ── Rule set ──────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Rules {
    /// Each entry is a bare domain (e.g. `"google.com"`).
    /// Matches the entry itself **and** all subdomains.
    suffixes: HashSet<String>,
}

impl Rules {
    /// Returns true if `host` equals any entry or is a subdomain of one.
    fn matches(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.');
        let mut start = 0;
        loop {
            let candidate = &host[start..];
            if self.suffixes.contains(candidate) {
                return true;
            }
            match candidate.find('.') {
                Some(dot) => start += dot + 1,
                None => return false,
            }
        }
    }

    #[cfg(feature = "rule-list-routing")]
    fn from_encoded(b64: &str) -> Result<Self, ProxyError> {
        use base64::Engine as _;
        let compact = b64
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect::<String>();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(compact)
            .map_err(|e| ProxyError::RuleList(format!("base64 decode: {e}")))?;
        let text = String::from_utf8(decoded)
            .map_err(|e| ProxyError::RuleList(format!("utf-8 decode: {e}")))?;
        Ok(Self::parse_abp(&text))
    }

    fn parse_abp(text: &str) -> Self {
        let mut suffixes = HashSet::new();
        for line in text.lines() {
            let line = line.trim();
            // Skip comments, headers, whitelist entries, and regex rules.
            if line.is_empty()
                || line.starts_with('!')
                || line.starts_with('[')
                || line.starts_with("@@")
                || line.starts_with('/')
            {
                continue;
            }
            if let Some(domain) = extract_domain(line)
                && !domain.is_empty()
            {
                suffixes.insert(domain.to_ascii_lowercase());
            }
        }
        Self { suffixes }
    }
}

/// Extract a bare domain from an ABP rule, returning `None` for unsupported
/// patterns (wildcards, path-level rules, regex).
fn extract_domain(rule: &str) -> Option<&str> {
    // ||example.com^  →  suffix-match "example.com"
    if let Some(rest) = rule.strip_prefix("||") {
        let d = rest.trim_end_matches('^').trim_end_matches('/');
        return if d.contains('/') || d.contains('*') {
            None
        } else {
            Some(d)
        };
    }
    // |https://example.com/path  →  extract "example.com"
    if let Some(rest) = rule.strip_prefix('|') {
        let without_scheme = rest
            .strip_prefix("https://")
            .or_else(|| rest.strip_prefix("http://"))
            .unwrap_or(rest);
        let host = without_scheme.split('/').next()?.split(':').next()?;
        return Some(host);
    }
    // Plain bare domain (no wildcards, paths, or query strings).
    if !rule.contains('*') && !rule.contains('/') && !rule.contains('=') && rule.contains('.') {
        return Some(rule);
    }
    None
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Routes GFW-blocked domains through `upstream`; everything else goes direct.
///
/// Rules are hot-swappable: call [`RuleListRouter::update_encoded`] without
/// taking the proxy offline.
pub struct RuleListRouter {
    rules: RwLock<Arc<Rules>>,
    upstream: UpstreamConfig,
}

impl RuleListRouter {
    /// Construct from a base64-encoded ABP-compatible rule-list payload.
    #[cfg(feature = "rule-list-routing")]
    pub fn from_encoded(b64: &str, upstream: UpstreamConfig) -> Result<Self, ProxyError> {
        Ok(Self {
            rules: RwLock::new(Arc::new(Rules::from_encoded(b64)?)),
            upstream,
        })
    }

    /// Atomically replace the active rule set (zero downtime).
    #[cfg(feature = "rule-list-routing")]
    pub fn update_encoded(&self, b64: &str) -> Result<(), ProxyError> {
        let new_rules = Arc::new(Rules::from_encoded(b64)?);
        *self.rules.write().unwrap() = new_rules;
        Ok(())
    }

    /// The upstream used for matched (blocked) domains.
    pub fn upstream(&self) -> &UpstreamConfig {
        &self.upstream
    }
}

impl ProxyRouter for RuleListRouter {
    fn route(&self, host: &str, _port: u16) -> Result<RouteDecision, ProxyError> {
        let matched = self.rules.read().unwrap().matches(host);
        Ok(RouteDecision::Upstream(if matched {
            self.upstream.clone()
        } else {
            UpstreamConfig::Direct
        }))
    }
}

// ── Fetch ─────────────────────────────────────────────────────────────────────

/// Fetch the raw (base64-encoded) gfwlist payload via `socks5` upstream.
///
/// Connects directly to the SOCKS5 server using the system TLS trust store,
/// bypassing the local proxy so the fetch always succeeds regardless of the
/// current routing rules.
#[cfg(feature = "rule-list-routing")]
pub async fn fetch_encoded(socks5: &UpstreamConfig) -> Result<String, ProxyError> {
    fetch_encoded_from_url(DEFAULT_RULE_LIST_URL, socks5).await
}

/// Fetch the raw (base64-encoded) gfwlist payload from a custom HTTPS source URL
/// via `socks5` upstream.
#[cfg(feature = "rule-list-routing")]
pub async fn fetch_encoded_from_url(
    source_url: &str,
    socks5: &UpstreamConfig,
) -> Result<String, ProxyError> {
    let (host, port, path) = parse_source_url(source_url)?;
    let stream = connect_upstream(socks5, &host, port).await?;

    // TLS using the system trust store (Security.framework on Apple,
    // SChannel on Windows, OpenSSL on Linux/Android).
    let native_cx = native_tls::TlsConnector::new()
        .map_err(|e| ProxyError::RuleList(format!("TLS init: {e}")))?;
    let cx = tokio_native_tls::TlsConnector::from(native_cx);
    let mut tls = cx
        .connect(&host, stream)
        .await
        .map_err(|e| ProxyError::RuleList(format!("TLS handshake: {e}")))?;

    let host_header = if port == 443 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_header}\r\nAccept: text/plain\r\nConnection: close\r\n\r\n"
    );
    tls.write_all(req.as_bytes())
        .await
        .map_err(ProxyError::Io)?;

    let mut buf = Vec::new();
    tls.read_to_end(&mut buf).await.map_err(ProxyError::Io)?;

    // Strip HTTP response headers.
    let sep = b"\r\n\r\n";
    let body_start = buf
        .windows(sep.len())
        .position(|w| w == sep)
        .ok_or_else(|| ProxyError::RuleList("no HTTP header separator in response".into()))?
        + sep.len();

    let body = String::from_utf8(buf[body_start..].to_vec())
        .map_err(|e| ProxyError::RuleList(format!("response body utf-8: {e}")))?;

    if body.trim().is_empty() {
        return Err(ProxyError::RuleList("empty response body".into()));
    }

    Ok(body)
}
