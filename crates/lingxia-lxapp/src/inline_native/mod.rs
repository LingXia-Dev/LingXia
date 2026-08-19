//! Inline native root protocol: identity, atomic commit, geometry, lease.
//!
//! Platform paint (HWND / UIView / TextureView) lives outside this module so
//! unit tests can drive the real apply functions.

mod apply;
mod geometry;
mod host;
mod lease;
mod resource;
mod types;
mod video;

pub use apply::{
    ApplyCommitOutcome, HostCapabilities, RootRegistry, apply_root_commit, evaluate_root_ready,
};
pub use geometry::{
    GeometryPageState, PendingGeometry, apply_geometry_snapshot, flush_pending_geometry,
    last_applied_geometry_revision,
};
pub use host::{IslandSession, IslandVideoNode, is_island_action, parse_applied_revision};
pub use lease::{
    DEFAULT_LEASE_DURATION_MS, LeasePhase, LeaseState, NEGOTIATION_TIMEOUT_MS, host_can_display,
    host_grant_lease, host_on_accept, host_on_renew, host_on_renew_accept, host_revoke_lease,
    host_tick_lease, view_can_show_fallback, view_on_grant, view_on_renew_granted,
    view_send_accept,
};
pub use resource::{media_urls_from_command_options, media_urls_from_props, validate_media_urls};
pub use types::{
    ALLOWED_HOST_KINDS, GeometryResultRoot, GeometryRootStatus, HostFactoryKind, NativeError,
    NativeErrorCode, NativeGeometryResult, NativeGeometrySnapshot, NativeGeometrySnapshotNode,
    NativeGeometrySnapshotRoot, NativeNode, NativeRootAck, NativeRootCommit,
    NativeRootLeaseMessage, NativeRootOperation, NodeRef, Rect, RootLifecycle, RootRef,
    ScrollChain, ScrollChainAncestor,
};
pub use video::{
    VideoCommand, VideoCommandOutcome, VideoCommandQueue, VideoCommandRequest,
    VideoControlDescriptor, VideoControlsSemanticSnapshot, apply_video_command,
    apply_video_controls_snapshot,
};

#[cfg(test)]
mod tests;
