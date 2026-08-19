#![allow(clippy::result_large_err)]

use super::apply::RootRegistry;
use super::types::{ErrorScope, NativeError, NativeErrorCode, NodeRef, RootLifecycle};
use serde::{Deserialize, Serialize};

const MAX_QUEUED_COMMANDS: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "camelCase")]
pub enum VideoCommand {
    Play,
    Pause,
    Stop,
    Seek { seconds: f64 },
    EnterFullscreen,
    ExitFullscreen,
    SetStreamSource { options: serde_json::Value },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCommandRequest {
    pub action: String,
    pub owner: NodeRef,
    pub request_id: String,
    pub command: VideoCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoCommandOutcome {
    Applied { request_id: String },
    Queued { request_id: String },
    Rejected(Box<NativeError>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoControlDescriptor {
    pub control_id: String,
    pub label: String,
    pub role: String,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoControlsSemanticSnapshot {
    pub action: String,
    pub owner: NodeRef,
    pub revision: u64,
    pub controls: Vec<VideoControlDescriptor>,
}

#[derive(Debug, Default, Clone)]
pub struct VideoCommandQueue {
    items: Vec<VideoCommandRequest>,
}

impl VideoCommandQueue {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

pub fn apply_video_command(
    registry: &RootRegistry,
    queue: &mut VideoCommandQueue,
    request: VideoCommandRequest,
) -> VideoCommandOutcome {
    let Some(state) = registry.get(&request.owner.root()) else {
        return VideoCommandOutcome::Rejected(Box::new(command_error(
            &request,
            NativeErrorCode::RootDestroyed,
            "video command owner root is not mounted",
        )));
    };
    match state.lifecycle {
        RootLifecycle::Destroyed | RootLifecycle::Failed | RootLifecycle::Unavailable => {
            return VideoCommandOutcome::Rejected(Box::new(command_error(
                &request,
                if state.lifecycle == RootLifecycle::Destroyed {
                    NativeErrorCode::RootDestroyed
                } else {
                    NativeErrorCode::CommandFailed
                },
                "video command rejected because the root is not runnable",
            )));
        }
        RootLifecycle::Negotiating | RootLifecycle::Mounting => {
            if queue.items.len() >= MAX_QUEUED_COMMANDS {
                return VideoCommandOutcome::Rejected(Box::new(command_error(
                    &request,
                    NativeErrorCode::CommandFailed,
                    "video command queue overflow; newest command rejected",
                )));
            }
            queue.items.push(request.clone());
            return VideoCommandOutcome::Queued {
                request_id: request.request_id,
            };
        }
        RootLifecycle::Ready => {}
    }
    if !state.nodes.contains_key(&request.owner.node_key) {
        return VideoCommandOutcome::Rejected(Box::new(command_error(
            &request,
            NativeErrorCode::CommandFailed,
            "video command owner node is not in the applied tree",
        )));
    }
    let Some(node) = state.nodes.get(&request.owner.node_key) else {
        return VideoCommandOutcome::Rejected(Box::new(command_error(
            &request,
            NativeErrorCode::CommandFailed,
            "video command owner node is not in the applied tree",
        )));
    };
    if node.kind != "video" || node.node_ref.node_epoch != request.owner.node_epoch {
        return VideoCommandOutcome::Rejected(Box::new(command_error(
            &request,
            NativeErrorCode::CommandFailed,
            "video command identity does not match a video node",
        )));
    }
    if let VideoCommand::Seek { seconds } = request.command
        && !seconds.is_finite()
    {
        return VideoCommandOutcome::Rejected(Box::new(command_error(
            &request,
            NativeErrorCode::InvalidProps,
            "seek seconds must be finite",
        )));
    }
    VideoCommandOutcome::Applied {
        request_id: request.request_id,
    }
}

pub fn apply_video_controls_snapshot(
    last_revision: u64,
    snapshot: &VideoControlsSemanticSnapshot,
) -> Result<u64, NativeError> {
    if snapshot.revision <= last_revision {
        return Err(NativeError {
            code: NativeErrorCode::InvalidProps,
            scope: ErrorScope::Node,
            recoverable: true,
            root: snapshot.owner.root(),
            node: Some(snapshot.owner.clone()),
            message: "controls snapshot revision must increase".into(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    for control in &snapshot.controls {
        if control.control_id.is_empty() || !seen.insert(control.control_id.clone()) {
            return Err(NativeError {
                code: NativeErrorCode::InvalidProps,
                scope: ErrorScope::Node,
                recoverable: true,
                root: snapshot.owner.root(),
                node: Some(snapshot.owner.clone()),
                message: "controlId must be unique and non-empty".into(),
            });
        }
        if control.role == "slider" {
            let (Some(min), Some(max), Some(value)) = (control.min, control.max, control.value)
            else {
                return Err(NativeError {
                    code: NativeErrorCode::InvalidProps,
                    scope: ErrorScope::Node,
                    recoverable: true,
                    root: snapshot.owner.root(),
                    node: Some(snapshot.owner.clone()),
                    message: "slider descriptor requires finite min/max/value".into(),
                });
            };
            if !min.is_finite()
                || !max.is_finite()
                || !value.is_finite()
                || min > value
                || value > max
            {
                return Err(NativeError {
                    code: NativeErrorCode::InvalidProps,
                    scope: ErrorScope::Node,
                    recoverable: true,
                    root: snapshot.owner.root(),
                    node: Some(snapshot.owner.clone()),
                    message: "slider descriptor value is out of range".into(),
                });
            }
        }
    }
    Ok(snapshot.revision)
}

fn command_error(
    request: &VideoCommandRequest,
    code: NativeErrorCode,
    message: &str,
) -> NativeError {
    NativeError {
        code,
        scope: ErrorScope::Node,
        recoverable: true,
        root: request.owner.root(),
        node: Some(request.owner.clone()),
        message: message.to_string(),
    }
}
