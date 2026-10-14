//! Wire contract between the coordinator and each platform backend.
//!
//! Commands are fire-and-forget and must be applied in order. Every outcome a
//! command can have comes back as an event; `req` ties a load or seek result
//! to the request that asked for it, so a superseded request's late answer is
//! ignored.

use super::types::{Focus, InterruptionPhase, InterruptionReason, RouteOutput, RouteReason};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Session output on the built-in route. External devices always win.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionOutput {
    #[default]
    Speaker,
    Earpiece,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "op", rename_all_fields = "camelCase")]
pub enum Command {
    /// `tickMs`: cadence of `player.progress` while playing.
    #[serde(rename = "player.create")]
    PlayerCreate { id: u64, tick_ms: u32 },
    /// Replace the current item. `uri` is an absolute local path or an
    /// http(s) URL. Answered by `player.loaded` or `player.error` with `req`.
    #[serde(rename = "player.load")]
    PlayerLoad {
        id: u64,
        req: u64,
        uri: String,
        headers: BTreeMap<String, String>,
        mime_type: Option<String>,
        start_ms: u64,
    },
    /// Answered by `player.playing` once audio renders.
    #[serde(rename = "player.play")]
    PlayerPlay { id: u64 },
    #[serde(rename = "player.pause")]
    PlayerPause { id: u64 },
    /// Answered by `player.seeked` with `req`.
    #[serde(rename = "player.seek")]
    PlayerSeek { id: u64, req: u64, position_ms: u64 },
    /// Release the item and its decoder; the player stays usable.
    #[serde(rename = "player.unload")]
    PlayerUnload { id: u64 },
    /// Effective volume (mute already applied), 0..1.
    #[serde(rename = "player.volume")]
    PlayerVolume { id: u64, volume: f64 },
    /// Pitch-preserving rate, 0.5..2.
    #[serde(rename = "player.rate")]
    PlayerRate { id: u64, rate: f64 },
    #[serde(rename = "player.dispose")]
    PlayerDispose { id: u64 },
    /// `path` is an absolute local file. Answered by `sound.loaded` or
    /// `sound.error` with `req`.
    #[serde(rename = "sound.load")]
    SoundLoad { id: u64, req: u64, path: String },
    /// `pan` is -1 (left) ..1 (right). A non-looping voice that finishes
    /// reports `sound.ended`.
    #[serde(rename = "sound.play")]
    SoundPlay {
        id: u64,
        voice: u64,
        volume: f64,
        rate: f64,
        pan: f64,
        #[serde(rename = "loop")]
        looping: bool,
    },
    #[serde(rename = "sound.stop")]
    SoundStop { id: u64, voice: u64 },
    #[serde(rename = "sound.dispose")]
    SoundDispose { id: u64 },
    /// The whole platform session. `focus` is the strongest live requirement
    /// (`None` when inactive); `obeySilentSwitch` holds only when every live
    /// participant obeys; `proximity` asks for near/far monitoring.
    #[serde(rename = "session")]
    Session {
        active: bool,
        focus: Option<Focus>,
        obey_silent_switch: bool,
        output: SessionOutput,
        proximity: bool,
    },
    /// Pause a native video that lost focus, as if its user paused it.
    #[serde(rename = "video.pause")]
    VideoPause { component_id: String },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "event", rename_all_fields = "camelCase")]
pub enum NativeEvent {
    #[serde(rename = "player.loaded")]
    PlayerLoaded {
        id: u64,
        req: u64,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    #[serde(rename = "player.playing")]
    PlayerPlaying {
        id: u64,
        #[serde(default)]
        position_ms: Option<u64>,
    },
    /// Native paused on its own (or confirms a pause) at `positionMs`.
    #[serde(rename = "player.paused")]
    PlayerPaused {
        id: u64,
        #[serde(default)]
        position_ms: Option<u64>,
    },
    #[serde(rename = "player.seeked")]
    PlayerSeeked { id: u64, req: u64, position_ms: u64 },
    #[serde(rename = "player.progress")]
    PlayerProgress {
        id: u64,
        position_ms: u64,
        #[serde(default)]
        buffered_ms: Option<u64>,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    #[serde(rename = "player.buffering")]
    PlayerBuffering { id: u64, buffering: bool },
    #[serde(rename = "player.ended")]
    PlayerEnded { id: u64 },
    /// `req` names the load that failed; absent for a playback failure.
    #[serde(rename = "player.error")]
    PlayerError {
        id: u64,
        #[serde(default)]
        req: Option<u64>,
        code: String,
        #[serde(default)]
        message: String,
    },
    #[serde(rename = "sound.loaded")]
    SoundLoaded { id: u64, req: u64, duration_ms: u64 },
    #[serde(rename = "sound.error")]
    SoundError {
        id: u64,
        req: u64,
        code: String,
        #[serde(default)]
        message: String,
    },
    #[serde(rename = "sound.ended")]
    SoundEnded { id: u64, voice: u64 },
    /// Current output. `lost` marks the previous device going away
    /// (unplugged, Bluetooth gone, becoming noisy).
    #[serde(rename = "route")]
    Route {
        output: RouteOutput,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        reason: RouteReason,
        #[serde(default)]
        lost: bool,
    },
    /// An external interruption. `shouldResume` is the OS hint on `ended`.
    #[serde(rename = "interruption")]
    Interruption {
        phase: InterruptionPhase,
        #[serde(default)]
        reason: InterruptionReason,
        #[serde(default)]
        should_resume: bool,
    },
    #[serde(rename = "proximity")]
    Proximity { near: bool },
    /// A native video became audible (`playing`) or stopped being audible.
    #[serde(rename = "video")]
    Video { component_id: String, playing: bool },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_serialize_with_op_and_camel_case_fields() {
        let json = serde_json::to_value(Command::PlayerSeek {
            id: 3,
            req: 9,
            position_ms: 1500,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"op": "player.seek", "id": 3, "req": 9, "positionMs": 1500})
        );
        let json = serde_json::to_value(Command::Session {
            active: true,
            focus: Some(Focus::Duck),
            obey_silent_switch: false,
            output: SessionOutput::Earpiece,
            proximity: true,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"op": "session", "active": true, "focus": "duck",
                "obeySilentSwitch": false, "output": "earpiece", "proximity": true})
        );
        let json = serde_json::to_value(Command::SoundPlay {
            id: 1,
            voice: 2,
            volume: 1.0,
            rate: 1.0,
            pan: -0.5,
            looping: true,
        })
        .unwrap();
        assert_eq!(json["loop"], true);
        assert_eq!(json["op"], "sound.play");
    }

    #[test]
    fn events_parse_with_defaults() {
        let event: NativeEvent =
            serde_json::from_str(r#"{"event":"route","output":"bluetooth","name":"AirPods"}"#)
                .unwrap();
        assert_eq!(
            event,
            NativeEvent::Route {
                output: RouteOutput::Bluetooth,
                name: Some("AirPods".into()),
                reason: RouteReason::Other,
                lost: false,
            }
        );
        let event: NativeEvent = serde_json::from_str(
            r#"{"event":"player.error","id":4,"code":"network","message":"offline"}"#,
        )
        .unwrap();
        assert!(matches!(event, NativeEvent::PlayerError { req: None, .. }));
        let event: NativeEvent =
            serde_json::from_str(r#"{"event":"interruption","phase":"ended","shouldResume":true}"#)
                .unwrap();
        assert_eq!(
            event,
            NativeEvent::Interruption {
                phase: InterruptionPhase::Ended,
                reason: InterruptionReason::System,
                should_resume: true,
            }
        );
    }
}
