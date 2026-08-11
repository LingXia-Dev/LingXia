//! Runtime-neutral media contracts.
//!
//! Playback owns the existing provider/session registry and accepts both video
//! and audio frames. Capture and other device input belong outside this crate.

#[cfg(feature = "playback")]
pub mod playback;
