use serde::{Deserialize, Serialize};

pub const ALLOWED_HOST_KINDS: &[&str] = &["root", "view", "text", "tappable", "slider", "video"];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootRef {
    pub surface_instance_id: String,
    pub page_instance_id: String,
    pub document_instance_id: String,
    pub root_key: String,
    pub root_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRef {
    pub surface_instance_id: String,
    pub page_instance_id: String,
    pub document_instance_id: String,
    pub root_key: String,
    pub root_epoch: u64,
    pub node_key: String,
    pub node_epoch: u64,
}

impl NodeRef {
    pub fn root(&self) -> RootRef {
        RootRef {
            surface_instance_id: self.surface_instance_id.clone(),
            page_instance_id: self.page_instance_id.clone(),
            document_instance_id: self.document_instance_id.clone(),
            root_key: self.root_key.clone(),
            root_epoch: self.root_epoch,
        }
    }

    pub fn same_root(&self, root: &RootRef) -> bool {
        self.surface_instance_id == root.surface_instance_id
            && self.page_instance_id == root.page_instance_id
            && self.document_instance_id == root.document_instance_id
            && self.root_key == root.root_key
            && self.root_epoch == root.root_epoch
    }
}

impl RootRef {
    pub fn same_generation(&self, other: &RootRef) -> bool {
        self.surface_instance_id == other.surface_instance_id
            && self.page_instance_id == other.page_instance_id
            && self.document_instance_id == other.document_instance_id
            && self.root_key == other.root_key
            && self.root_epoch == other.root_epoch
    }

    pub fn same_document(&self, other: &RootRef) -> bool {
        self.surface_instance_id == other.surface_instance_id
            && self.page_instance_id == other.page_instance_id
            && self.document_instance_id == other.document_instance_id
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeNode {
    #[serde(rename = "ref")]
    pub node_ref: NodeRef,
    pub kind: String,
    pub parent: Option<NodeRef>,
    pub order: u32,
    pub author_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation_id: Option<String>,
    #[serde(default)]
    pub props: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum NativeRootOperation {
    Mount {
        node: NativeNode,
    },
    Update {
        node: NodeRef,
        patch: serde_json::Value,
    },
    Reparent {
        node: NodeRef,
        parent: Option<NodeRef>,
    },
    Reorder {
        node: NodeRef,
        order: u32,
    },
    Unmount {
        node: NodeRef,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRootCommit {
    pub action: String,
    pub root: RootRef,
    pub base_revision: u64,
    pub revision: u64,
    pub operations: Vec<NativeRootOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRootAck {
    Applied {
        root: RootRef,
        revision: u64,
    },
    ResyncRequired {
        root: RootRef,
        last_applied_revision: u64,
    },
    Quiesced {
        root: RootRef,
        lease_id: Option<String>,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeError {
    pub code: NativeErrorCode,
    pub scope: ErrorScope,
    pub recoverable: bool,
    pub root: RootRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeRef>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorScope {
    Node,
    Root,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeErrorCode {
    #[serde(rename = "NATIVE_ROOT_UNAVAILABLE")]
    RootUnavailable,
    #[serde(rename = "NATIVE_ROOT_INCOMPATIBLE")]
    RootIncompatible,
    #[serde(rename = "NATIVE_ROOT_INVALID_STRUCTURE")]
    InvalidStructure,
    #[serde(rename = "NATIVE_ROOT_FAILED")]
    RootFailed,
    #[serde(rename = "NATIVE_ROOT_UNSUPPORTED_LAYOUT")]
    UnsupportedLayout,
    #[serde(rename = "NATIVE_COMPONENT_INVALID_PROPS")]
    InvalidProps,
    #[serde(rename = "NATIVE_COMPONENT_MOUNT_FAILED")]
    MountFailed,
    #[serde(rename = "NATIVE_COMPONENT_COMMAND_FAILED")]
    CommandFailed,
    #[serde(rename = "NATIVE_ROOT_UNSUPPORTED_STYLE")]
    UnsupportedStyle,
    #[serde(rename = "NATIVE_ROOT_DESTROYED")]
    RootDestroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootLifecycle {
    Negotiating,
    Mounting,
    Ready,
    Unavailable,
    Failed,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeometryRootStatus {
    Applied,
    PendingTree,
    StaleGeneration,
    StaleRevision,
    IdentityInvalid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollChainAncestor {
    pub key: String,
    pub viewport_rect: Rect,
    pub offset_x: f64,
    pub offset_y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollChain {
    pub chain_key: String,
    pub ancestors: Vec<ScrollChainAncestor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeometrySnapshotRoot {
    #[serde(rename = "ref")]
    pub root_ref: RootRef,
    pub basis_tree_revision: u64,
    pub root_order: u32,
    pub chain_key: String,
    pub content_rect: Rect,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeometrySnapshotNode {
    #[serde(rename = "ref")]
    pub node_ref: NodeRef,
    pub chain_key: String,
    pub content_rect: Rect,
    #[serde(default)]
    pub clip_stack: Vec<serde_json::Value>,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeometrySnapshot {
    pub action: String,
    pub surface_instance_id: String,
    pub page_instance_id: String,
    pub document_instance_id: String,
    pub revision: u64,
    pub coordinate_space: String,
    pub roots: Vec<NativeGeometrySnapshotRoot>,
    pub nodes: Vec<NativeGeometrySnapshotNode>,
    pub chains: Vec<ScrollChain>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryResultRoot {
    pub root: RootRef,
    pub basis_tree_revision: u64,
    pub last_applied_tree_revision: u64,
    pub status: GeometryRootStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeometryResult {
    pub action: String,
    pub surface_instance_id: String,
    pub page_instance_id: String,
    pub document_instance_id: String,
    pub revision: u64,
    pub roots: Vec<GeometryResultRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum NativeRootLeaseMessage {
    #[serde(rename = "root.leaseGranted")]
    LeaseGranted {
        root: RootRef,
        lease_id: String,
        sequence: u64,
        lease_duration_ms: u64,
    },
    #[serde(rename = "root.leaseAccept")]
    LeaseAccept {
        root: RootRef,
        lease_id: String,
        sequence: u64,
    },
    #[serde(rename = "root.leaseActive")]
    LeaseActive {
        root: RootRef,
        lease_id: String,
        sequence: u64,
    },
    #[serde(rename = "root.leaseRenew")]
    LeaseRenew {
        root: RootRef,
        lease_id: String,
        sequence: u64,
    },
    #[serde(rename = "root.leaseRenewGranted")]
    LeaseRenewGranted {
        root: RootRef,
        lease_id: String,
        sequence: u64,
        lease_duration_ms: u64,
    },
    #[serde(rename = "root.leaseRenewAccept")]
    LeaseRenewAccept {
        root: RootRef,
        lease_id: String,
        sequence: u64,
    },
    #[serde(rename = "root.leaseRevoked")]
    LeaseRevoked {
        root: RootRef,
        lease_id: String,
        sequence: u64,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostFactoryKind {
    Root,
    View,
    Text,
    Tappable,
    Slider,
    Video,
}

impl HostFactoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::View => "view",
            Self::Text => "text",
            Self::Tappable => "tappable",
            Self::Slider => "slider",
            Self::Video => "video",
        }
    }

    pub fn parse(kind: &str) -> Option<Self> {
        match kind {
            "root" => Some(Self::Root),
            "view" => Some(Self::View),
            "text" => Some(Self::Text),
            "tappable" => Some(Self::Tappable),
            "slider" => Some(Self::Slider),
            "video" => Some(Self::Video),
            _ => None,
        }
    }
}
