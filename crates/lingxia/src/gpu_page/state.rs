//! Logic state between the lxapp's GPU types and the View's: moved, never serialized.
#[cfg(target_os = "android")]
use lingxia_view_android::lingxia_view;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use lingxia_view_apple::lingxia_view;
#[cfg(target_env = "ohos")]
use lingxia_view_harmony::lingxia_view;

use lingxia_view::session::state::{PatchOp, StateSignal, StateWrite};
use lxapp::{GpuPatchOp, GpuStateSignal, GpuStateWrite};

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
