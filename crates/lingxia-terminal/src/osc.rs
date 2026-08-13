//! OSC and Kitty APC tapping plus semantic OSC parsing.
//!
//! `alacritty_terminal`'s vte parser dispatches only the OSC sequences it
//! understands internally (title, colors, hyperlink, clipboard); semantic
//! sequences like OSC 7/9/99/133/777 are silently dropped. This module
//! taps the raw byte stream alongside the parser so the crate can turn
//! those sequences into typed events for hosts.

/// A completed OSC sequence located in a fed byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TappedOsc {
    /// Byte offset of the leading `ESC ]` within the current feed call;
    /// `0` when the sequence started in an earlier feed call.
    pub start: usize,
    /// Byte offset one past the terminator within the current feed call.
    pub end: usize,
    /// OSC body: the bytes between `ESC ]` and the BEL/ST terminator.
    pub body: Vec<u8>,
}

/// A completed control string that the shared engine handles alongside the
/// VT parser. Kitty bodies omit the leading G protocol discriminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TappedControl {
    Osc(TappedOsc),
    KittyGraphics {
        start: usize,
        end: usize,
        body: Vec<u8>,
    },
    ClearScreen {
        start: usize,
        end: usize,
        /// ED parameter (`0`, `2`, or the ConPTY clear-buffer marker `3`).
        mode: u8,
        /// Full lines erased immediately before ConPTY's ED 3 marker.
        erased_lines: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum TapState {
    #[default]
    Ground,
    Esc,
    Csi,
    Osc,
    OscEsc,
    Apc,
    ApcEsc,
    String,
    StringEsc,
}

/// Incrementally scans terminal output for OSC and Kitty APC sequences.
///
/// Tracks CSI and DCS/SOS/PM/APC string states so `ESC ]` bytes inside
/// them are not misread as OSC starts. State persists across feed calls;
/// offsets in the emitted sequences are relative to the bytes of the
/// call that completed them.
#[derive(Default)]
pub struct OscTap {
    state: TapState,
    buffer: Vec<u8>,
    csi: Vec<u8>,
    linefeeds: Vec<usize>,
    cell_size_queries: usize,
    erased_lines: usize,
}

impl OscTap {
    /// Maximum control body retained; oversized sequences are dropped to
    /// bound memory when a peer floods the stream.
    const MAX_OSC_BODY: usize = 256 * 1024;
    const MAX_APC_BODY: usize = 96 * 1024 * 1024;

    #[cfg(test)]
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<TappedOsc> {
        self.feed_controls(bytes)
            .into_iter()
            .filter_map(|control| match control {
                TappedControl::Osc(osc) => Some(osc),
                TappedControl::KittyGraphics { .. } | TappedControl::ClearScreen { .. } => None,
            })
            .collect()
    }

    pub fn feed_controls(&mut self, bytes: &[u8]) -> Vec<TappedControl> {
        let mut tapped = Vec::new();
        self.linefeeds.clear();
        let mut start = 0_usize;
        for (index, &byte) in bytes.iter().enumerate() {
            if byte == b'\n'
                && matches!(self.state, TapState::Ground | TapState::Esc | TapState::Csi)
            {
                self.linefeeds.push(index);
            }
            match self.state {
                TapState::Ground => match byte {
                    0x1b => {
                        start = index;
                        self.state = TapState::Esc;
                    }
                    0x9b => {
                        start = index;
                        self.csi.clear();
                        self.state = TapState::Csi;
                    }
                    0x9d => {
                        self.buffer.clear();
                        start = index;
                        self.state = TapState::Osc;
                    }
                    0x9f => {
                        self.buffer.clear();
                        start = index;
                        self.state = TapState::Apc;
                    }
                    0x90 | 0x98 | 0x9e => self.state = TapState::String,
                    b'\r' | b'\n' => {}
                    _ => self.erased_lines = 0,
                },
                TapState::Esc => {
                    if byte == 0x1b {
                        start = index;
                    }
                    self.state = match byte {
                        b'[' => {
                            self.csi.clear();
                            TapState::Csi
                        }
                        b']' => {
                            self.buffer.clear();
                            TapState::Osc
                        }
                        b'_' => {
                            self.buffer.clear();
                            TapState::Apc
                        }
                        b'P' | b'X' | b'^' => TapState::String,
                        0x1b => TapState::Esc,
                        _ => TapState::Ground,
                    };
                }
                TapState::Csi => {
                    if byte == 0x1b {
                        start = index;
                        self.state = TapState::Esc;
                    } else if (0x40..=0x7e).contains(&byte) {
                        if byte == b't' && self.csi == b"16" {
                            self.cell_size_queries += 1;
                        }
                        let mode = match self.csi.as_slice() {
                            b"" | b"0" => 0,
                            b"2" => 2,
                            b"3" => 3,
                            _ => u8::MAX,
                        };
                        if byte == b'K' && mode == 0 {
                            self.erased_lines = self.erased_lines.saturating_add(1);
                        } else if byte == b'J'
                            && (mode == 0 || mode == 2 || (mode == 3 && self.erased_lines > 0))
                        {
                            tapped.push(TappedControl::ClearScreen {
                                start,
                                end: index + 1,
                                mode,
                                erased_lines: self.erased_lines,
                            });
                            self.erased_lines = 0;
                        } else if !(matches!(byte, b'H' | b'f')
                            || (matches!(byte, b'h' | b'l') && self.csi == b"?25"))
                        {
                            self.erased_lines = 0;
                        }
                        self.csi.clear();
                        self.state = TapState::Ground;
                    } else if self.csi.len() < 32 {
                        self.csi.push(byte);
                    }
                }
                TapState::Osc => match byte {
                    0x07 | 0x9c => {
                        tapped.push(TappedControl::Osc(TappedOsc {
                            start,
                            end: index + 1,
                            body: std::mem::take(&mut self.buffer),
                        }));
                        self.state = TapState::Ground;
                    }
                    0x1b => self.state = TapState::OscEsc,
                    _ => {
                        if self.buffer.len() >= Self::MAX_OSC_BODY {
                            self.buffer.clear();
                            self.state = TapState::String;
                        } else {
                            self.buffer.push(byte);
                        }
                    }
                },
                TapState::OscEsc => {
                    if byte == b'\\' {
                        tapped.push(TappedControl::Osc(TappedOsc {
                            start,
                            end: index + 1,
                            body: std::mem::take(&mut self.buffer),
                        }));
                        self.state = TapState::Ground;
                    } else {
                        self.buffer.clear();
                        self.state = if byte == 0x1b {
                            TapState::Esc
                        } else {
                            TapState::Ground
                        };
                    }
                }
                TapState::Apc => match byte {
                    0x9c => {
                        push_apc(&mut tapped, start, index + 1, &mut self.buffer);
                        self.state = TapState::Ground;
                    }
                    0x1b => self.state = TapState::ApcEsc,
                    _ => {
                        if self.buffer.len() >= Self::MAX_APC_BODY {
                            self.buffer.clear();
                            self.state = TapState::String;
                        } else {
                            self.buffer.push(byte);
                        }
                    }
                },
                TapState::ApcEsc => {
                    if byte == b'\\' {
                        push_apc(&mut tapped, start, index + 1, &mut self.buffer);
                        self.state = TapState::Ground;
                    } else {
                        self.buffer.clear();
                        self.state = if byte == 0x1b {
                            TapState::Esc
                        } else {
                            TapState::Ground
                        };
                    }
                }
                TapState::String => match byte {
                    0x9c => self.state = TapState::Ground,
                    0x1b => self.state = TapState::StringEsc,
                    _ => {}
                },
                TapState::StringEsc => {
                    self.state = match byte {
                        b'\\' => TapState::Ground,
                        0x1b => TapState::StringEsc,
                        _ => TapState::String,
                    };
                }
            }
        }
        tapped
    }

    pub fn take_linefeeds(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.linefeeds)
    }

    pub fn take_cell_size_queries(&mut self) -> usize {
        std::mem::take(&mut self.cell_size_queries)
    }
}

fn push_apc(tapped: &mut Vec<TappedControl>, start: usize, end: usize, buffer: &mut Vec<u8>) {
    let body = std::mem::take(buffer);
    if let Some(body) = body.strip_prefix(b"G") {
        tapped.push(TappedControl::KittyGraphics {
            start,
            end,
            body: body.to_vec(),
        });
    }
}

/// Progress state from OSC 9;4 (ConEmu/Windows Terminal semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscProgress {
    Idle,
    Running { percent: Option<u8> },
    Paused { percent: Option<u8> },
    Failed,
}

/// Semantic interpretation of one OSC body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscSemantic {
    /// OSC 7: current working directory (file:// URL).
    Cwd(String),
    /// OSC 9;4: progress / task state.
    Progress(OscProgress),
    /// OSC 9 / 99 / 777: user-facing notification.
    Notification {
        title: Option<String>,
        body: String,
    },
    /// OSC 133 A/B/C/D shell integration marks.
    PromptStart,
    InputStart,
    OutputStart,
    CommandFinished {
        exit_code: Option<i32>,
    },
}

/// Maximum bytes kept from a notification payload.
pub const NOTIFICATION_PAYLOAD_LIMIT: usize = 1024;

/// Interpret a tapped OSC body. Returns `None` for sequences without
/// crate-level semantics (title, colors, hyperlink, clipboard — those
/// are handled by the terminal emulator itself).
pub fn parse_osc(body: &[u8]) -> Option<OscSemantic> {
    let mut parts = body.split(|&b| b == b';');
    let code = parts.next()?;
    match code {
        b"7" => {
            let url = std::str::from_utf8(parts.next()?).ok()?;
            cwd_from_file_url(url).map(OscSemantic::Cwd)
        }
        b"9" => {
            let first = parts.next()?;
            if first == b"4" {
                let state = parse_u8(parts.next()?)?;
                let percent = parts.next().and_then(parse_u8).map(|p| p.min(100));
                let progress = match state {
                    0 => OscProgress::Idle,
                    1 => OscProgress::Running { percent },
                    2 => OscProgress::Failed,
                    3 => OscProgress::Running { percent: None },
                    4 => OscProgress::Paused { percent },
                    _ => return None,
                };
                Some(OscSemantic::Progress(progress))
            } else {
                let body = sanitize_payload(first)?;
                Some(OscSemantic::Notification { title: None, body })
            }
        }
        b"99" => {
            // Kitty notification: OSC 99 ; metadata ; payload.
            let _metadata = parts.next()?;
            let body = sanitize_payload(parts.next()?)?;
            Some(OscSemantic::Notification { title: None, body })
        }
        b"777" => {
            // OSC 777 ; notify ; title ; body.
            match parts.next()? {
                b"notify" => {
                    let title = sanitize_payload(parts.next()?)?;
                    let body = sanitize_payload(parts.next().unwrap_or_default())?;
                    Some(OscSemantic::Notification {
                        title: Some(title),
                        body,
                    })
                }
                _ => None,
            }
        }
        b"133" => match parts.next()? {
            b"A" => Some(OscSemantic::PromptStart),
            b"B" => Some(OscSemantic::InputStart),
            b"C" => Some(OscSemantic::OutputStart),
            b"D" => {
                let exit_code = parts.next().and_then(|raw| {
                    let raw = std::str::from_utf8(raw).ok()?;
                    raw.trim().parse::<i32>().ok()
                });
                Some(OscSemantic::CommandFinished { exit_code })
            }
            _ => None,
        },
        _ => None,
    }
}

/// Extract the path from an OSC 7 file:// URL, percent-decoded.
fn cwd_from_file_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    // Skip the authority component; an empty or localhost authority is
    // the common case, anything else is a remote path we keep verbatim.
    let path_start = rest.find('/')?;
    let path = &rest[path_start..];
    percent_decode(path.as_bytes())
}

pub fn percent_decode(bytes: &[u8]) -> Option<String> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes.get(index + 1..index + 3)?;
            let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            out.push(value);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Sanitize a protocol payload destined for host UI: strip C0/C1
/// control characters and cap the length.
pub fn sanitize_payload(bytes: &[u8]) -> Option<String> {
    let bytes = &bytes[..bytes.len().min(NOTIFICATION_PAYLOAD_LIMIT * 4)];
    let text = String::from_utf8_lossy(bytes);
    let filtered: String = text
        .chars()
        .filter(|ch| !ch.is_control() || *ch == ' ')
        .take(NOTIFICATION_PAYLOAD_LIMIT)
        .collect();
    let trimmed = filtered.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn parse_u8(bytes: &[u8]) -> Option<u8> {
    std::str::from_utf8(bytes).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap_all(chunks: &[&[u8]]) -> Vec<TappedOsc> {
        let mut tap = OscTap::default();
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(tap.feed(chunk));
        }
        out
    }

    #[test]
    fn taps_bel_and_st_terminated_osc() {
        let tapped = tap_all(&[b"plain \x1b]7;file:///tmp\x07 mid \x1b]133;A\x1b\\ end"]);
        assert_eq!(tapped.len(), 2);
        assert_eq!(tapped[0].body, b"7;file:///tmp");
        assert_eq!(tapped[0].start, 6);
        assert_eq!(tapped[1].body, b"133;A");
    }

    #[test]
    fn ignores_osc_like_bytes_inside_strings_and_csi() {
        let dcs = b"\x1bP q \x1b]7;file:///nope\x07 \x1b\\";
        let csi = b"\x1b[31;1mred \x1b[0m";
        let tapped = tap_all(&[dcs, csi]);
        assert!(tapped.is_empty(), "no OSC inside DCS/CSI: {tapped:?}");
    }

    #[test]
    fn osc_spanning_feed_chunks_completes() {
        let tapped = tap_all(&[b"\x1b]133;", b"D;9\x07"]);
        assert_eq!(tapped.len(), 1);
        assert_eq!(
            parse_osc(&tapped[0].body),
            Some(OscSemantic::CommandFinished { exit_code: Some(9) })
        );
    }

    #[test]
    fn drops_oversized_osc_body() {
        let mut bytes = b"\x1b]7;".to_vec();
        bytes.extend(std::iter::repeat_n(b'a', OscTap::MAX_OSC_BODY + 16));
        bytes.push(0x07);
        let tapped = tap_all(&[&bytes]);
        assert!(tapped.is_empty());
    }

    #[test]
    fn parses_cwd_with_percent_decoding() {
        assert_eq!(
            parse_osc(b"7;file://host/Users/a%20b/work"),
            Some(OscSemantic::Cwd("/Users/a b/work".to_string()))
        );
        assert_eq!(
            parse_osc(b"7;file:///C:/work"),
            Some(OscSemantic::Cwd("/C:/work".to_string()))
        );
        assert_eq!(parse_osc(b"7;https://nope"), None);
    }

    #[test]
    fn parses_progress_states() {
        assert_eq!(
            parse_osc(b"9;4;1;42"),
            Some(OscSemantic::Progress(OscProgress::Running {
                percent: Some(42)
            }))
        );
        assert_eq!(
            parse_osc(b"9;4;3;0"),
            Some(OscSemantic::Progress(OscProgress::Running {
                percent: None
            }))
        );
        assert_eq!(
            parse_osc(b"9;4;2;55"),
            Some(OscSemantic::Progress(OscProgress::Failed))
        );
        assert_eq!(
            parse_osc(b"9;4;0"),
            Some(OscSemantic::Progress(OscProgress::Idle))
        );
    }

    #[test]
    fn parses_notifications_sanitized() {
        assert_eq!(
            parse_osc(b"9;build done"),
            Some(OscSemantic::Notification {
                title: None,
                body: "build done".to_string()
            })
        );
        assert_eq!(
            parse_osc(b"777;notify;title;body text"),
            Some(OscSemantic::Notification {
                title: Some("title".to_string()),
                body: "body text".to_string()
            })
        );
        assert_eq!(
            parse_osc(b"99;i=1:d=0;kitty note"),
            Some(OscSemantic::Notification {
                title: None,
                body: "kitty note".to_string()
            })
        );
        // Control characters are stripped, empty payloads dropped.
        assert_eq!(
            parse_osc(b"9;a\x07b"),
            Some(OscSemantic::Notification {
                title: None,
                body: "ab".to_string()
            })
        );
        assert_eq!(parse_osc(b"9;\x01\x02"), None);
    }

    #[test]
    fn parses_semantic_marks() {
        assert_eq!(parse_osc(b"133;A"), Some(OscSemantic::PromptStart));
        assert_eq!(parse_osc(b"133;A;aid=x"), Some(OscSemantic::PromptStart));
        assert_eq!(parse_osc(b"133;B"), Some(OscSemantic::InputStart));
        assert_eq!(parse_osc(b"133;C"), Some(OscSemantic::OutputStart));
        assert_eq!(
            parse_osc(b"133;D;0"),
            Some(OscSemantic::CommandFinished { exit_code: Some(0) })
        );
        assert_eq!(
            parse_osc(b"133;D"),
            Some(OscSemantic::CommandFinished { exit_code: None })
        );
    }

    #[test]
    fn ignores_non_semantic_osc() {
        assert_eq!(parse_osc(b"0;title"), None);
        assert_eq!(parse_osc(b"8;;https://example.com"), None);
        assert_eq!(parse_osc(b"52;c;aGVsbG8="), None);
    }

    #[test]
    fn taps_kitty_apc_across_chunks_without_exposing_it_as_osc() {
        let mut tap = OscTap::default();
        assert!(tap.feed_controls(b"before\x1b_Gi=4,m=1;").is_empty());
        let controls = tap.feed_controls(b"AAAA\x1b\\after");
        assert_eq!(controls.len(), 1);
        assert!(matches!(
            &controls[0],
            TappedControl::KittyGraphics { body, .. } if body == b"i=4,m=1;AAAA"
        ));
    }

    #[test]
    fn taps_clear_screen_in_stream_order() {
        let mut tap = OscTap::default();
        let controls = tap.feed_controls(b"before\x1b[2Jafter\x9b2J");

        assert_eq!(
            controls,
            vec![
                TappedControl::ClearScreen {
                    start: 6,
                    end: 10,
                    mode: 2,
                    erased_lines: 0,
                },
                TappedControl::ClearScreen {
                    start: 15,
                    end: 18,
                    mode: 2,
                    erased_lines: 0,
                },
            ]
        );
    }

    #[test]
    fn taps_clear_screen_across_chunks() {
        let mut tap = OscTap::default();
        assert!(tap.feed_controls(b"\x1b[").is_empty());
        assert_eq!(
            tap.feed_controls(b"2Jafter"),
            vec![TappedControl::ClearScreen {
                start: 0,
                end: 2,
                mode: 2,
                erased_lines: 0,
            }]
        );
    }

    #[test]
    fn taps_erase_to_end_and_ignores_other_modes() {
        let mut tap = OscTap::default();
        assert_eq!(
            tap.feed_controls(b"\x1b[J\x1b[0J\x1b[1J\x1b[3J\x1b[2K"),
            vec![
                TappedControl::ClearScreen {
                    start: 0,
                    end: 3,
                    mode: 0,
                    erased_lines: 0,
                },
                TappedControl::ClearScreen {
                    start: 3,
                    end: 7,
                    mode: 0,
                    erased_lines: 0,
                },
            ]
        );
    }

    #[test]
    fn taps_conpty_console_clear_across_chunks() {
        let mut tap = OscTap::default();
        assert!(tap.feed_controls(b"\x1b[H\x1b[K\r\n\x1b[K\r\n").is_empty());
        assert_eq!(
            tap.feed_controls(b"\x1b[H\x1b[3J"),
            vec![TappedControl::ClearScreen {
                start: 3,
                end: 7,
                mode: 3,
                erased_lines: 2,
            }]
        );
    }
}
