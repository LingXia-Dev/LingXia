//! Host-wide audio: one coordinator owns the platform audio session, and
//! every player, sound, and native video registers with it.
//!
//! Logic (`lx.audio`) talks to [`AudioRuntime`]; platforms implement
//! `lingxia_platform::traits::audio::AudioBackend` and speak [`protocol`].

mod coordinator;
pub mod protocol;
mod runtime;
mod session;
mod types;

pub use coordinator::{
    HostEvent, ItemReason, OwnerId, PlayerCommand, PlayerEvent, PlayerOptions, PlayerSnapshot,
    QueueEntry, Reply, SoundEvent, SoundOptions, VoiceParams,
};
pub use runtime::{AudioRuntime, PlayerHandle, SoundHandle, get, start};
pub use types::{
    AudioError, AudioErrorCode, Focus, Interruption, InterruptionPhase, InterruptionReason,
    OutputRequest, PlayerStatus, Repeat, Route, RouteChange, RouteOutput, RouteReason,
};

/// Whether this build has a native audio backend.
pub const fn available() -> bool {
    cfg!(any(target_os = "android", target_os = "windows"))
}
