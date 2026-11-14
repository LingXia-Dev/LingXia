//! `<video>` events and commands: player states become the page's typed events, and its
//! handles' commands reach the players.
use super::ViewSession;
use crate::engine::JSContext;
use crate::protocol::{field, tag};
use crate::scene::video::{Status, VideoOp};

/// `__lingxiaVideoCommand(player, op, value)`: `""` once applied, else the error message.
/// `__lingxiaVideoOrientation(orientation)`: what the full-screen layer asks of the window,
/// `""` for nothing.
pub(super) fn install(
    context: &JSContext,
    scope: u64,
    orientation: impl Fn(Option<crate::scene::video::Orientation>) + 'static,
) -> Result<(), String> {
    let request = rong::quickjs::JSFunc::new(context, move |text: String| {
        match crate::scene::video::Orientation::parse(&text) {
            Some(value) => {
                orientation(value);
                String::new()
            }
            None => format!("UI_VIDEO: unknown orientation {text}"),
        }
    })
    .map_err(|e| e.to_string())?;
    context
        .global()
        .set("__lingxiaVideoOrientation", request)
        .map_err(|e| e.to_string())?;
    let command = rong::quickjs::JSFunc::new(
        context,
        move |player: String, op: String, value: f64, text: String| {
            let Some(op) = VideoOp::parse(&op, value, &text) else {
                return format!("VIDEO_SOURCE: unknown command {op}");
            };
            match crate::renderer::video_command(&crate::scene::video::key(scope, &player), op) {
                Ok(()) => String::new(),
                Err(error) => error.message,
            }
        },
    )
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
    // Fires when the frames on screen change rendition, not when a switch is asked for.
    let mode = before.quality != now.quality && (now.quality == "auto" || now.quality == now.shown);
    if before.ready_state == "ready" && !now.shown.is_empty() && (before.shown != now.shown || mode)
    {
        out.push(
            json!({"type": "qualitychange", "quality": now.shown, "auto": now.quality == "auto"}),
        );
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
    /// a handler ran or a poster must give way to the first frame. They go to the latest commit's
    /// handlers: a page that commits every frame (a spinner) is always a commit past its last
    /// present.
    pub fn dispatch_video_events(&mut self, hold_time: bool) -> Result<bool, String> {
        let generation = self.generation;
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

impl ViewSession {
    pub(crate) fn video_scope(&self) -> u64 {
        self.renderer.videos.scope()
    }

    /// A Logic video command for the page's player `player`: a `useVideo` name, else a
    /// `<video id>`.
    pub(crate) fn video_request(
        &mut self,
        player: &str,
        command: &crate::session::logic::VideoCommand,
    ) -> Result<serde_json::Value, crate::session::logic::BridgeError> {
        use crate::session::logic::VideoCommand as C;
        let fail = |error: crate::scene::video::VideoError| crate::session::logic::BridgeError {
            code: serde_json::Value::from(error.code),
            message: Some(error.message),
            data: Some(Box::new(
                serde_json::json!({ "recoverable": error.recoverable }),
            )),
        };
        let records = self.input_records.as_deref().unwrap_or_default();
        let named = format!("name:{player}");
        let videos = || records.iter().filter(|r| r.tag == tag::VIDEO);
        let wire = videos()
            .find_map(|r| r.string(field::VIDEO_PLAYER).filter(|p| *p == named))
            .or_else(|| {
                videos()
                    .find(|r| r.string(field::ELEMENT_ID) == Some(player))
                    .and_then(|r| r.string(field::VIDEO_PLAYER))
            })
            .ok_or_else(|| {
                fail(crate::scene::video::VideoError::new(
                    "VIDEO_NO_PLAYER",
                    format!(
                        "no <video> on this page plays \"{player}\"; name its player useVideo(\"{player}\") or give the <video> id=\"{player}\""
                    ),
                ))
            })?;
        let key = self.renderer.videos.key(wire);
        let unsupported = |what: &str| {
            fail(crate::scene::video::VideoError::new(
                "VIDEO_UNSUPPORTED",
                format!("{what} is not available on GPU pages yet"),
            ))
        };
        let ops = match command {
            C::Play => vec![VideoOp::Play],
            C::Pause => vec![VideoOp::Pause],
            C::Stop => vec![VideoOp::Pause, VideoOp::Seek(0.0)],
            C::Seek(seconds) if seconds.is_finite() && *seconds >= 0.0 => {
                vec![VideoOp::Seek(*seconds)]
            }
            C::SetPlaybackRate(rate) if (0.25..=4.0).contains(rate) => {
                vec![VideoOp::SetRate(*rate as f32)]
            }
            C::SetVolume(volume) if (0.0..=1.0).contains(volume) => {
                vec![VideoOp::SetVolume(*volume as f32)]
            }
            C::SetMuted(muted) => vec![VideoOp::SetMuted(*muted)],
            C::SetQuality(quality) if quality == "auto" => vec![VideoOp::SetQuality(None)],
            C::SetQuality(quality) if !quality.is_empty() => {
                vec![VideoOp::SetQuality(Some(quality.clone()))]
            }
            C::RequestFullScreen => return Err(unsupported("requestFullScreen")),
            C::ExitFullScreen => return Err(unsupported("exitFullScreen")),
            C::SetStreamSource => return Err(unsupported("setStreamSource")),
            other => {
                return Err(fail(crate::scene::video::VideoError::new(
                    "VIDEO_SOURCE",
                    format!("{other:?} is out of range"),
                )));
            }
        };
        for op in ops {
            crate::renderer::video_command(&key, op).map_err(fail)?;
        }
        Ok(serde_json::Value::Null)
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
    fn a_quality_change_reports_the_label_that_shows() {
        let ready = Status {
            ready_state: "ready",
            shown: "720p".into(),
            ..Status::default()
        };
        let pinned = Status {
            quality: "360p".into(),
            ..ready.clone()
        };
        assert!(events(&ready, &pinned).is_empty());
        let shows = Status {
            shown: "360p".into(),
            ..pinned.clone()
        };
        let change = &events(&pinned, &shows)[0];
        assert_eq!(
            (&change["quality"], &change["auto"]),
            (&"360p".into(), &false.into())
        );
        let loading = Status {
            ready_state: "loading",
            ..Status::default()
        };
        assert!(
            events(&loading, &ready)
                .iter()
                .all(|e| e["type"] != "qualitychange")
        );
    }

    #[test]
    fn sources_need_unique_labels_other_than_auto() {
        use crate::scene::video::variants;
        let two = r#"[{"src":"a.mp4","label":"360p"},{"src":"b.mp4","label":"720p"}]"#;
        assert_eq!(variants(two).unwrap().len(), 2);
        assert!(variants(r#"[{"src":"a.mp4","label":"x"},{"src":"b.mp4","label":"x"}]"#).is_none());
        assert!(variants(r#"[{"src":"a.mp4","label":"auto"}]"#).is_none());
        assert!(variants("[]").is_none());
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
