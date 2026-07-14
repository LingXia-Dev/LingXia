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
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            // One visible slot per aside kind: lxapp, browser, native.
            max_asides_expanded: 3,
            max_asides_medium: 1,
            max_asides_compact: 0,
            main_min_width: 360.0,
            aside_min_width: 240.0,
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
) -> (SurfaceGraph, Decision) {
    let mut next = graph.clone();

    match request.role {
        // main / float are not bound by the split limit.
        Role::Main | Role::Float => {
            next.insert(request);
            (next, Decision::Accepted)
        }
        Role::Aside => {
            let max = policy.max_asides(size_class);
            let has_main = !next.mains().is_empty();

            // Can't be an aside without a primary, and no side-by-side room in
            // compact (max==0): in both cases promote to a main.
            if !has_main || max == 0 {
                let decision = if max == 0 {
                    Decision::FullScreenFallback
                } else {
                    Decision::DowngradedRole
                };
                let promoted_id = request.id.clone();
                next.insert(promote_to_main(request));
                next.set_active_main(&promoted_id);
                next.set_focus(&promoted_id);
                return (next, decision);
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
                return (next, Decision::MergedIntoTabs);
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
                return (next, Decision::MergedIntoTabs);
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
            if joins_open_slot {
                (next, Decision::MergedIntoTabs)
            } else {
                (next, Decision::Accepted)
            }
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
    graph
        .surfaces()
        .iter()
        .find(|s| s.id != exclude_id && s.role == Role::Aside && web_url(s) == Some(url))
        .map(|s| s.id.clone())
}
