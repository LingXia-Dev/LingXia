//! `<video>` events and commands: player states become the page's typed events, and its
//! handles' commands reach the players.
use super::ViewSession;
use crate::engine::JSContext;
use crate::protocol::{field, tag};
use crate::scene::video::{Status, VideoOp};

/// `__lingxiaVideoCommand(player, op, value)`: `""` once applied, else the error message.
pub(super) fn install(context: &JSContext, scope: u64) -> Result<(), String> {
    let command =
        rong::quickjs::JSFunc::new(context, move |player: String, op: String, value: f64| {
            let Some(op) = VideoOp::parse(&op, value) else {
                return format!("VIDEO_SOURCE: unknown command {op}");
            };
            match crate::renderer::video_command(&crate::scene::video::key(scope, &player), op) {
                Ok(()) => String::new(),
                Err(error) => error.message,
            }
        })
        .map_err(|e| e.to_string())?;
    context
        .global()
        .set("__lingxiaVideoCommand", command)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The events between two states of one player, in the order a viewer sees them.
fn events(before: &Status, now: &Status) -> Vec<serde_json::Value> {
    use serde_json::json;
    let mut out = Vec::new();
    if before.duration.is_none()
        && let Some(duration) = now.duration
    {
        out.push(
            json!({"type": "loadedmetadata", "duration": duration, "videoWidth": now.video_width,
            "videoHeight": now.video_height, "live": duration.is_infinite()}),
        );
    }
    if before.error.is_none() && now.error.is_some() {
        out.push(json!({"type": "error"}));
    }
    if before.paused && !now.paused {
        out.push(json!({"type": "play"}));
    }
    let moving = |s: &Status| !s.paused && !s.buffering && s.ready_state == "ready";
    if !now.buffering && moving(now) && !moving(before) {
        out.push(json!({"type": "playing"}));
    }
    if !before.buffering && now.buffering && !now.paused {
        out.push(json!({"type": "waiting"}));
    }
    if (now.current_time - before.current_time).abs() >= 0.25 || (now.ended && !before.ended) {
        out.push(json!({"type": "timeupdate", "currentTime": now.current_time}));
    }
    if !before.paused && now.paused {
        out.push(json!({"type": "pause", "reason": "api"}));
    }
    if !before.ended && now.ended {
        out.push(json!({"type": "ended"}));
    }
    if before.playback_rate != now.playback_rate {
        out.push(json!({"type": "ratechange", "playbackRate": now.playback_rate}));
    }
    if before.volume != now.volume || before.muted != now.muted {
        out.push(json!({"type": "volumechange", "volume": now.volume, "muted": now.muted}));
    }
    out
}

impl ViewSession {
    /// Players whose state moved since it was last reported get their views' events; true when
    /// a handler ran or a poster must give way to the first frame.
    pub fn dispatch_video_events(
        &mut self,
        generation: crate::session::lifecycle::CommitTicket,
        hold_time: bool,
    ) -> Result<bool, String> {
        if generation != self.generation {
            return Ok(false);
        }
        let records = self.input_records.clone().unwrap_or_default();
        let mut statuses = std::collections::HashMap::new();
        let mut views = Vec::new();
        let mut repaint = false;
        for record in records.iter().filter(|r| r.tag == tag::VIDEO) {
            let key = self
                .renderer
                .videos
                .key(record.string(field::VIDEO_PLAYER).unwrap_or_default());
            let Some(now) = crate::renderer::video_status(&key) else {
                continue;
            };
            let before = self.video_reported.get(&key).cloned().unwrap_or_default();
            if before == now || (hold_time && time_only(&before, &now)) {
                continue;
            }
            repaint |= record.string(field::VIDEO_POSTER).is_some()
                && (before.ready_state == "ready") != (now.ready_state == "ready");
            if let Some(handle) = record
                .fields
                .iter()
                .find(|(id, _)| *id == field::ON_VIDEO_EVENT)
                .and_then(|(_, value)| value.as_u64())
            {
                views.push((key.clone(), handle));
            }
            statuses.insert(key, (before, now));
        }
        let mut changed = repaint;
        for (key, handle) in views {
            let (before, now) = &statuses[&key];
            let payload = serde_json::json!({"status": now, "events": events(before, now)});
            changed |= self.text_event(generation, handle, &payload.to_string())?;
        }
        for (key, (_, now)) in statuses {
            self.video_reported.insert(key, now);
        }
        Ok(changed)
    }
}

/// Only playback time moved: nothing a reader acts on besides a time label.
fn time_only(before: &Status, now: &Status) -> bool {
    let mut held = now.clone();
    held.current_time = before.current_time;
    held.buffered = before.buffered;
    held == *before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_reports_metadata_then_play_then_ended() {
        let empty = Status::default();
        let ready = Status {
            ready_state: "ready",
            duration: Some(6.0),
            paused: false,
            ..Status::default()
        };
        let kinds = |events: Vec<serde_json::Value>| {
            events
                .into_iter()
                .map(|e| e["type"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds(events(&empty, &ready)),
            ["loadedmetadata", "play", "playing"]
        );
        let ended = Status {
            current_time: 6.0,
            paused: true,
            ended: true,
            ..ready.clone()
        };
        assert_eq!(
            kinds(events(&ready, &ended)),
            ["timeupdate", "pause", "ended"]
        );
        let failed = Status {
            error: Some(crate::scene::video::VideoError::new(
                "VIDEO_NOT_FOUND",
                "clip.mp4",
            )),
            ..empty.clone()
        };
        assert_eq!(kinds(events(&empty, &failed)), ["error"]);
    }

    #[test]
    fn a_scroll_holds_back_time_but_not_a_pause() {
        let playing = Status {
            ready_state: "ready",
            paused: false,
            current_time: 1.0,
            ..Status::default()
        };
        let later = Status {
            current_time: 1.5,
            buffered: 3.0,
            ..playing.clone()
        };
        assert!(time_only(&playing, &later));
        let paused = Status {
            paused: true,
            ..later
        };
        assert!(!time_only(&playing, &paused));
    }
}
