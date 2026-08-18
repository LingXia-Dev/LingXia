//! Runtime-neutral media contracts.
//!
//! Playback owns the existing provider/session registry and accepts both video
//! and audio frames. Realtime capture is a separate feature that depends on
//! the platform capture contract and never on `lingxia-device-io`.

#[cfg(feature = "playback")]
pub mod playback;

#[cfg(feature = "capture")]
pub mod capture;
