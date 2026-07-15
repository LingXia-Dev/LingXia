//! Host arbitration (§3.4): a deterministic, **infallible** pure function that
//! decides how an open-request lands in the graph and always leaves it valid.
//! `(graph, request, policy) -> (graph', decision)`.
//!
//! Rejection (caps / permissions) is a *separate* host-policy gate applied
//! before this core runs (§3.1); the pure layout core never rejects — it
//! resolves by degrading.

use serde::{Deserialize, Serialize};

use crate::graph::SurfaceGraph;
use crate::layout::SizeClass;
use crate::model::{Role, Surface, SurfaceContent};

/// Structured outcome of a request (§3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Accepted,
    DowngradedRole,
    ReplacedExisting,
    FullScreenFallback,
    /// The aside joined an already-open slot of its kind as a tab (or, for a
    /// repeat web URL / aside id, focused the existing tab). Asides form one
    /// region per content kind — lxapp, browser, native — with tabs inside.
    MergedIntoTabs,
}

/// Result of opening a surface after reuse/arbitration has resolved its
/// identity and role. Callers must bind handles to `resolved_surface_id`, not
/// to the request id: a reused URL aside keeps the original runtime id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOutcome {
    pub decision: Decision,
    pub resolved_surface_id: crate::model::SurfaceId,
    pub resolved_role: Role,
    /// The requested aside must cover the main rather than dock. Compact
    /// always has this form; physical admission can add it at wider classes.
    pub overlay: bool,
}

impl OpenOutcome {
    fn new(
        decision: Decision,
        resolved_surface_id: crate::model::SurfaceId,
        resolved_role: Role,
        overlay: bool,
    ) -> Self {
        Self {
            decision,
            resolved_surface_id,
            resolved_role,
            overlay,
        }
    }
}

impl PartialEq<Decision> for OpenOutcome {
    fn eq(&self, other: &Decision) -> bool {
        self.decision == *other
    }
}

/// Tunable arbitration policy. Defaults are the spec's cross-platform defaults.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub max_asides_expanded: usize,
    pub max_asides_medium: usize,
    pub max_asides_compact: usize,
    /// Physical admission tokens (§3.3): a slot is admitted only when the main
    /// keeps `main_min_width` and each admitted left/right slot keeps
    /// `aside_min_width` within the container. Size class is the *ceiling*, not
    /// a guarantee — a technically-expanded but narrow window admits fewer.
    pub main_min_width: f64,
    pub aside_min_width: f64,
    /// Sidebar allocation happens before aside admission. Compact has no
    /// sidebar; Medium uses the rail token and Expanded uses the full token.
    pub sidebar_expanded_width: f64,
    pub sidebar_medium_width: f64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            // One visible slot per aside kind: lxapp, browser, native.
            max_asides_expanded: 3,
            max_asides_medium: 1,
            // Compact shows one active slot full-screen over the main. This is
            // a projection limit, not dock capacity.
            max_asides_compact: 1,
            main_min_width: 360.0,
            aside_min_width: 240.0,
            sidebar_expanded_width: 220.0,
            sidebar_medium_width: 56.0,
        }
    }
}

impl Policy {
    pub fn max_asides(&self, size_class: SizeClass) -> usize {
        match size_class {
            SizeClass::Expanded => self.max_asides_expanded,
            SizeClass::Medium => self.max_asides_medium,
            SizeClass::Compact => self.max_asides_compact,
        }
    }
}

/// Run arbitration. Pure: clones the graph, applies the resolved request, and
/// returns the new graph plus the decision. The result graph is always valid.
pub fn arbitrate(
    graph: &SurfaceGraph,
    request: Surface,
    policy: &Policy,
    size_class: SizeClass,
) -> (SurfaceGraph, OpenOutcome) {
    let mut next = graph.clone();
    let request_id = request.id.clone();

    match request.role {
        // main / float are not bound by the split limit.
        Role::Main | Role::Float => {
            let role = request.role;
            next.insert(request);
            (
                next,
                OpenOutcome::new(Decision::Accepted, request_id, role, false),
            )
        }
        Role::Aside => {
            let max = policy.max_asides(size_class);
            let has_main = !next.mains().is_empty();

            // An aside needs a primary. Compact still preserves the aside role:
            // the skin projects it as a full-screen overlay over that primary.
            if !has_main {
                let promoted_id = request.id.clone();
                next.insert(promote_to_main(request));
                next.set_active_main(&promoted_id);
                next.set_focus(&promoted_id);
                return (
                    next,
                    OpenOutcome::new(Decision::DowngradedRole, promoted_id, Role::Main, false),
                );
            }

            // Asides group into ONE region (slot) per content kind — lxapp,
            // browser, native — and multiple contents of a kind live inside
            // that region as tabs. Opening a second content of an open kind
            // therefore never consumes extra budget and never evicts anything:
            // it joins the slot. Over-limit slots are hidden by the plan's
            // admission, not evicted from the graph.
            //
            // Web asides dedupe by URL — reopening a URL focuses the existing
            // tab instead of adding a duplicate.
            if let Some(url) = web_url(&request)
                && let Some(existing) = existing_web_aside_with_url(&next, &request.id, url)
            {
                next.set_focus(&existing);
                return (
                    next,
                    OpenOutcome::new(
                        Decision::MergedIntoTabs,
                        existing,
                        Role::Aside,
                        size_class == SizeClass::Compact,
                    ),
                );
            }
            // Reopening an existing aside id (an lxapp's appId, the terminal)
            // focuses its tab.
            if next
                .get(&request.id)
                .is_some_and(|existing| existing.role == Role::Aside)
            {
                let id = request.id.clone();
                next.insert(request);
                next.set_focus(&id);
                return (
                    next,
                    OpenOutcome::new(
                        Decision::MergedIntoTabs,
                        id,
                        Role::Aside,
                        size_class == SizeClass::Compact,
                    ),
                );
            }

            let slot = request.content.slot_kind();
            let open_kinds: std::collections::HashSet<crate::model::SlotKind> = next
                .asides()
                .iter()
                .map(|s| s.content.slot_kind())
                .collect();
            let joins_open_slot = open_kinds.contains(&slot);
            let id = request.id.clone();
            next.insert(request);
            next.set_focus(&id);
            let decision = if size_class == SizeClass::Compact {
                Decision::FullScreenFallback
            } else if joins_open_slot {
                Decision::MergedIntoTabs
            } else {
                Decision::Accepted
            };
            (
                next,
                OpenOutcome::new(
                    decision,
                    id,
                    Role::Aside,
                    size_class == SizeClass::Compact || max == 0,
                ),
            )
        }
    }
}

fn promote_to_main(mut request: Surface) -> Surface {
    request.role = Role::Main;
    request.placement.edge = None;
    request
}

/// The web URL of a surface, if it is web content.
fn web_url(surface: &Surface) -> Option<&str> {
    match &surface.content {
        SurfaceContent::Web { url } => Some(url.as_str()),
        _ => None,
    }
}

/// An existing web-content aside serving `url` (other than `exclude_id`).
fn existing_web_aside_with_url(
    graph: &SurfaceGraph,
    exclude_id: &str,
    url: &str,
) -> Option<String> {
    let key = normalize_initial_url(url);
    graph
        .surfaces()
        .iter()
        .find(|surface| {
            surface.id != exclude_id
                && surface.role == Role::Aside
                && web_url(surface).is_some_and(|candidate| normalize_initial_url(candidate) == key)
        })
        .map(|s| s.id.clone())
}

/// Stable key for a URL aside's initial URL. Navigation never mutates the
/// graph's stored URL, so reuse remains tied to the initial request. Query and
/// fragment bytes remain part of the key.
pub fn normalize_initial_url(raw: &str) -> String {
    let raw = raw.trim();
    let (before_fragment, fragment) = raw
        .split_once('#')
        .map_or((raw, None), |(head, tail)| (head, Some(tail)));
    let (before_query, query) = before_fragment
        .split_once('?')
        .map_or((before_fragment, None), |(head, tail)| (head, Some(tail)));
    let Some((scheme, rest)) = before_query.split_once("://") else {
        return raw.to_string();
    };
    let scheme = scheme.to_ascii_lowercase();
    let (authority, path) = rest
        .find('/')
        .map(|index| (&rest[..index], &rest[index..]))
        .unwrap_or((rest, "/"));
    let authority = normalize_authority(authority, &scheme);
    let mut normalized = format!("{scheme}://{authority}{path}");
    if let Some(query) = query {
        normalized.push('?');
        normalized.push_str(query);
    }
    if let Some(fragment) = fragment {
        normalized.push('#');
        normalized.push_str(fragment);
    }
    normalized
}

fn normalize_authority(authority: &str, scheme: &str) -> String {
    if let Some(rest) = authority.strip_prefix('[')
        && let Some((host, suffix)) = rest.split_once(']')
    {
        let suffix = suffix
            .strip_prefix(':')
            .filter(|port| !is_default_port(scheme, port))
            .map_or(String::new(), |port| format!(":{port}"));
        return format!("[{}]{suffix}", host.to_ascii_lowercase());
    }
    let (host, port) = authority
        .rsplit_once(':')
        .filter(|(_, port)| port.bytes().all(|byte| byte.is_ascii_digit()))
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    let suffix = port
        .filter(|port| !is_default_port(scheme, port))
        .map_or(String::new(), |port| format!(":{port}"));
    format!("{}{suffix}", host.to_ascii_lowercase())
}

fn is_default_port(scheme: &str, port: &str) -> bool {
    matches!((scheme, port), ("https", "443") | ("http", "80"))
}
