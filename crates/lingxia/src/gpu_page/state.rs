//! Logic state and View events between the lxapp's GPU types and the View's: moved, never serialized.
#[cfg(target_os = "android")]
use lingxia_view_android::lingxia_view;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use lingxia_view_apple::lingxia_view;
#[cfg(target_env = "ohos")]
use lingxia_view_harmony::lingxia_view;
#[cfg(windows)]
use lingxia_view_windows::lingxia_view;

pub(crate) use lingxia_view::session::logic::ToLogic;
use lingxia_view::session::logic::{BridgeError, ToView, UiRequest, VideoCommand, ViewRequest};
use lingxia_view::session::state::{PatchOp, StateSignal, StateWrite};
use lxapp::{
    GpuBridgeError, GpuPatchOp, GpuStateSignal, GpuStateWrite, GpuUiRequest, GpuVideoCommand,
    GpuViewEvent, GpuViewMessage, GpuViewRequest,
};

pub(crate) fn page_orientation(
    orientation: lingxia_view::scene::video::Orientation,
) -> lxapp::PageOrientation {
    match orientation {
        lingxia_view::scene::video::Orientation::Landscape => lxapp::PageOrientation::Landscape,
        lingxia_view::scene::video::Orientation::Portrait => lxapp::PageOrientation::Portrait,
    }
}

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
        ToLogic::Connected => GpuViewEvent::Connected,
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
                data: error.data.map(|data| *data),
            }),
        },
    }
}

fn error_to_view(error: GpuBridgeError) -> BridgeError {
    BridgeError {
        code: error.code,
        message: error.message,
        data: error.data.map(Box::new),
    }
}

pub(crate) fn message_to_view(message: GpuViewMessage) -> ToView {
    match message {
        GpuViewMessage::Reply { id, outcome } => ToView::Reply {
            id,
            outcome: outcome.map_err(error_to_view),
        },
        GpuViewMessage::StreamEvent { id, seq, payload } => {
            ToView::StreamEvent { id, seq, payload }
        }
        GpuViewMessage::ChannelAck { id, outcome } => ToView::ChannelAck {
            id,
            outcome: outcome.map_err(error_to_view),
        },
        GpuViewMessage::ChannelData { id, seq, payload } => {
            ToView::ChannelData { id, seq, payload }
        }
        GpuViewMessage::ChannelClose { id, code, reason } => {
            ToView::ChannelClose { id, code, reason }
        }
        GpuViewMessage::Request { id, request } => ToView::Request {
            id,
            request: match request {
                GpuViewRequest::Ui(request) => ViewRequest::Ui(match request {
                    GpuUiRequest::ShowToast(toast) => UiRequest::ShowToast {
                        title: toast.title,
                        icon: toast.icon,
                        duration: toast.duration,
                        mask: toast.mask,
                        position: toast.position,
                    },
                    GpuUiRequest::HideToast => UiRequest::HideToast,
                    GpuUiRequest::ShowModal(modal) => UiRequest::ShowModal {
                        title: modal.title,
                        content: modal.content,
                        show_cancel: modal.show_cancel,
                        cancel_text: modal.cancel_text,
                        confirm_text: modal.confirm_text,
                    },
                    GpuUiRequest::ShowActionSheet(sheet) => UiRequest::ShowActionSheet {
                        items: sheet.item_list,
                        cancel_text: sheet.cancel_text,
                    },
                }),
                GpuViewRequest::Video(request) => ViewRequest::Video {
                    player: request.player,
                    command: match request.command {
                        GpuVideoCommand::Play => VideoCommand::Play,
                        GpuVideoCommand::Pause => VideoCommand::Pause,
                        GpuVideoCommand::Stop => VideoCommand::Stop,
                        GpuVideoCommand::Seek { seconds } => VideoCommand::Seek(seconds),
                        GpuVideoCommand::RequestFullScreen => VideoCommand::RequestFullScreen,
                        GpuVideoCommand::ExitFullScreen => VideoCommand::ExitFullScreen,
                        GpuVideoCommand::SetStreamSource => VideoCommand::SetStreamSource,
                        GpuVideoCommand::SetQuality { quality } => {
                            VideoCommand::SetQuality(quality)
                        }
                        GpuVideoCommand::SetPlaybackRate { rate } => {
                            VideoCommand::SetPlaybackRate(rate)
                        }
                        GpuVideoCommand::SetMuted { muted } => VideoCommand::SetMuted(muted),
                        GpuVideoCommand::SetVolume { volume } => VideoCommand::SetVolume(volume),
                    },
                },
            },
        },
    }
}

/// Dev sessions: the View says how each module swap ended, for the swap waiting on it.
#[cfg(feature = "devtool")]
pub(crate) fn install_swap_reporter() {
    lingxia_view::session::host::set_swap_reporter(super::swap_reports::report);
}
