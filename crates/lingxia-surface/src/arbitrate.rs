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
use crate::model::{Edge, Role, Surface, SurfaceContent};

/// Structured outcome of a request (§3.1). A subset of the spec's full set;
/// `mergedIntoTabs` lands when tab-grouping is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Accepted,
    DowngradedRole,
    ReplacedExisting,
    FullScreenFallback,
}

/// Tunable arbitration policy. Defaults are the spec's cross-platform defaults.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub max_asides_expanded: usize,
    pub max_asides_medium: usize,
    pub max_asides_compact: usize,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_asides_expanded: 2,
            max_asides_medium: 1,
            max_asides_compact: 0,
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

            // A web-content (in-app browser) aside is a per-window singleton: a
            // new browser aside replaces any existing one, independent of the
            // generic aside cap. The browser aside is a single companion pane,
            // not an unbounded set.
            if is_web(&request)
                && let Some(victim) = existing_web_aside_id(&next, &request.id)
            {
                next.remove(&victim);
                next.insert(request);
                return (next, Decision::ReplacedExisting);
            }

            // Under the limit: accept as-is.
            if next.asides().len() < max {
                next.insert(request);
                return (next, Decision::Accepted);
            }

            // Over the limit: replace the oldest aside (preferring same edge).
            if let Some(victim) = oldest_aside_id(&next, request.placement.edge) {
                next.remove(&victim);
                next.insert(request);
                (next, Decision::ReplacedExisting)
            } else {
                next.insert(promote_to_main(request));
                (next, Decision::DowngradedRole)
            }
        }
    }
}

fn promote_to_main(mut request: Surface) -> Surface {
    request.role = Role::Main;
    request.placement.edge = None;
    request
}

fn is_web(surface: &Surface) -> bool {
    matches!(surface.content, SurfaceContent::Web { .. })
}

/// The existing web-content aside (if any) other than `exclude_id`.
fn existing_web_aside_id(graph: &SurfaceGraph, exclude_id: &str) -> Option<String> {
    graph
        .surfaces()
        .iter()
        .find(|s| s.id != exclude_id && s.role == Role::Aside && is_web(s))
        .map(|s| s.id.clone())
}

/// Oldest (insertion-order-first) aside, preferring the same edge if given.
fn oldest_aside_id(graph: &SurfaceGraph, edge: Option<Edge>) -> Option<String> {
    if let Some(edge) = edge
        && let Some(s) = graph
            .surfaces()
            .iter()
            .find(|s| s.role == Role::Aside && s.placement.edge == Some(edge))
    {
        return Some(s.id.clone());
    }
    graph
        .surfaces()
        .iter()
        .find(|s| s.role == Role::Aside)
        .map(|s| s.id.clone())
}
