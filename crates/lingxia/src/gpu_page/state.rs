//! Logic state and View events between the lxapp's GPU types and the View's: moved, never serialized.
#[cfg(target_os = "android")]
use lingxia_view_android::lingxia_view;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use lingxia_view_apple::lingxia_view;
#[cfg(target_env = "ohos")]
use lingxia_view_harmony::lingxia_view;

pub(crate) use lingxia_view::session::logic::ToLogic;
use lingxia_view::session::state::{PatchOp, StateSignal, StateWrite};
use lxapp::{GpuBridgeError, GpuPatchOp, GpuStateSignal, GpuStateWrite, GpuViewEvent};

pub(crate) fn to_view(write: GpuStateWrite) -> StateWrite {
    match write {
        GpuStateWrite::Snapshot { rev, state } => StateWrite::Snapshot { rev, state },
        GpuStateWrite::Patch { rev, ops, ack } => StateWrite::Patch {
            rev,
            ack,
            ops: ops
                .into_iter()
                .map(|op| match op {
                    GpuPatchOp::Add { path, value } => PatchOp::Add { path, value },
                    GpuPatchOp::Replace { path, value } => PatchOp::Replace { path, value },
                    GpuPatchOp::Remove { path } => PatchOp::Remove { path },
                })
                .collect(),
        },
    }
}

pub(crate) fn from_view(signal: StateSignal) -> GpuStateSignal {
    match signal {
        StateSignal::Acked(rev) => GpuStateSignal::Acked(rev),
        StateSignal::Resync => GpuStateSignal::Resync,
    }
}

pub(crate) fn event_from_view(message: ToLogic) -> GpuViewEvent {
    match message {
        ToLogic::Call {
            id,
            method,
            params,
            sent_us,
        } => GpuViewEvent::Call {
            id,
            method,
            params,
            sent_us,
        },
        ToLogic::Cancel { id } => GpuViewEvent::Cancel { id },
        ToLogic::ChannelOpen { id, topic, params } => {
            GpuViewEvent::ChannelOpen { id, topic, params }
        }
        ToLogic::ChannelData { id, payload } => GpuViewEvent::ChannelData { id, payload },
        ToLogic::ChannelClose { id, code, reason } => {
            GpuViewEvent::ChannelClose { id, code, reason }
        }
        ToLogic::Reply { id, outcome } => GpuViewEvent::Reply {
            id,
            outcome: outcome.map_err(|error| GpuBridgeError {
                code: error.code,
                message: error.message,
                data: error.data,
            }),
        },
    }
}
