//! CSS Grid track lists and line placements, parsed from bounded wire strings.
use taffy::prelude::*;
use taffy::style::{
    GridTemplateComponent, GridTemplateRepetition, MaxTrackSizingFunction, MinTrackSizingFunction,
    RepetitionCount,
};

/// Most explicit tracks one template may expand to.
pub(crate) const MAX_TRACKS: usize = 64;

fn invalid(token: &str) -> String {
    format!("UI_GRID: invalid track or line `{token}`")
}

/// Whitespace-separated tokens, keeping parenthesized groups whole.
fn tokens(source: &str) -> Result<Vec<&str>, String> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, None);
    for (at, c) in source.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return Err(invalid(source));
        }
        if c.is_whitespace() && depth == 0 {
            if let Some(from) = start.take() {
                out.push(&source[from..at]);
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if depth != 0 {
        return Err(invalid(source));
    }
    if let Some(from) = start {
        out.push(&source[from..]);
    }
    Ok(out)
}

fn call<'a>(token: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let inner = token
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')?;
    let mut parts = Vec::new();
    let (mut depth, mut from) = (0, 0);
    for (at, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[from..at].trim());
                from = at + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[from..].trim());
    Some(parts)
}

fn number(text: &str, unit: &str, max: f32) -> Option<f32> {
    text.strip_suffix(unit)?
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && (0.0..=max).contains(v))
}

fn min_function(token: &str) -> Result<MinTrackSizingFunction, String> {
    Ok(match token {
        "auto" => MinTrackSizingFunction::AUTO,
        "min-content" => MinTrackSizingFunction::MIN_CONTENT,
        "max-content" => MinTrackSizingFunction::MAX_CONTENT,
        "0" => MinTrackSizingFunction::length(0.0),
        _ => {
            if let Some(px) = number(token, "px", 16384.0) {
                MinTrackSizingFunction::length(px)
            } else if let Some(pct) = number(token, "%", 1000.0) {
                MinTrackSizingFunction::percent(pct / 100.0)
            } else {
                return Err(invalid(token));
            }
        }
    })
}

fn max_function(token: &str) -> Result<MaxTrackSizingFunction, String> {
    if let Some(fr) = number(token, "fr", 16384.0) {
        return Ok(MaxTrackSizingFunction::fr(fr));
    }
    Ok(min_function(token)?.into())
}

fn track(token: &str) -> Result<TrackSizingFunction, String> {
    if let Some(args) = call(token, "minmax") {
        let [min, max] = args.as_slice() else {
            return Err(invalid(token));
        };
        return Ok(minmax(min_function(min)?, max_function(max)?));
    }
    if let Some(args) = call(token, "fit-content") {
        let [limit] = args.as_slice() else {
            return Err(invalid(token));
        };
        return Ok(if let Some(px) = number(limit, "px", 16384.0) {
            fit_content(LengthPercentage::length(px))
        } else if let Some(pct) = number(limit, "%", 1000.0) {
            fit_content(LengthPercentage::percent(pct / 100.0))
        } else {
            return Err(invalid(token));
        });
    }
    // A bare flexible track is `minmax(auto, <flex>)`, as in CSS.
    if let Some(fr) = number(token, "fr", 16384.0) {
        return Ok(minmax(
            MinTrackSizingFunction::AUTO,
            MaxTrackSizingFunction::fr(fr),
        ));
    }
    let min = min_function(token)?;
    Ok(minmax(min, min.into()))
}

/// `grid-template-columns`/`-rows`: tracks, `repeat(N | auto-fill | auto-fit, …)`,
/// `minmax()`, `fit-content()`. Named lines and areas are not admitted.
pub(crate) fn template(source: &str) -> Result<Vec<GridTemplateComponent<String>>, String> {
    if source.len() > 1024 || source.trim().is_empty() {
        return Err(invalid(source));
    }
    let mut out = Vec::new();
    let mut expanded = 0;
    let mut automatic = false;
    for token in tokens(source)? {
        if let Some(args) = call(token, "repeat") {
            let [count, list] = args.as_slice() else {
                return Err(invalid(token));
            };
            let tracks = tokens(list)?
                .into_iter()
                .map(track)
                .collect::<Result<Vec<_>, _>>()?;
            let count = match *count {
                "auto-fill" => RepetitionCount::AutoFill,
                "auto-fit" => RepetitionCount::AutoFit,
                n => RepetitionCount::Count(
                    n.parse::<u16>()
                        .ok()
                        .filter(|n| (1..=MAX_TRACKS as u16).contains(n))
                        .ok_or_else(|| invalid(token))?,
                ),
            };
            if tracks.is_empty() {
                return Err(invalid(token));
            }
            if let RepetitionCount::Count(n) = count {
                expanded += usize::from(n) * tracks.len();
            } else {
                // One auto repetition per template, of tracks with a fixed size.
                if automatic || !tracks.iter().all(TrackSizingFunction::has_fixed_component) {
                    return Err(invalid(token));
                }
                automatic = true;
                expanded += tracks.len();
            }
            out.push(GridTemplateComponent::Repeat(GridTemplateRepetition {
                count,
                tracks,
                line_names: Vec::new(),
            }));
        } else {
            expanded += 1;
            out.push(GridTemplateComponent::Single(track(token)?));
        }
        if expanded > MAX_TRACKS {
            return Err(format!(
                "UI_GRID: more than {MAX_TRACKS} tracks in `{source}`"
            ));
        }
    }
    Ok(out)
}

/// `grid-auto-columns`/`-rows`: plain tracks, no repetition.
pub(crate) fn auto_tracks(source: &str) -> Result<Vec<TrackSizingFunction>, String> {
    if source.len() > 256 {
        return Err(invalid(source));
    }
    let tracks = tokens(source)?
        .into_iter()
        .map(track)
        .collect::<Result<Vec<_>, _>>()?;
    if tracks.is_empty() || tracks.len() > MAX_TRACKS {
        return Err(invalid(source));
    }
    Ok(tracks)
}

fn edge(token: &str) -> Result<GridPlacement, String> {
    if token == "auto" {
        return Ok(GridPlacement::Auto);
    }
    if let Some(n) = token.strip_prefix("span ") {
        return n
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|n| (1..=MAX_TRACKS as u16).contains(n))
            .map(span)
            .ok_or_else(|| invalid(token));
    }
    token
        .parse::<i16>()
        .ok()
        .filter(|n| *n != 0 && n.unsigned_abs() as usize <= MAX_TRACKS + 1)
        .map(line)
        .ok_or_else(|| invalid(token))
}

/// `grid-column`/`grid-row`: `start [/ end]` with lines, `span N` or `auto`.
pub(crate) fn placement(source: &str) -> Result<Line<GridPlacement>, String> {
    if source.len() > 64 {
        return Err(invalid(source));
    }
    let mut parts = source.split('/').map(str::trim);
    let start = edge(parts.next().unwrap_or_default())?;
    let end = parts
        .next()
        .map(edge)
        .transpose()?
        .unwrap_or(GridPlacement::Auto);
    if parts.next().is_some() {
        return Err(invalid(source));
    }
    Ok(Line { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_admit_css_track_functions_within_bounds() {
        assert_eq!(template("repeat(3, minmax(0, 1fr))").unwrap().len(), 1);
        assert_eq!(
            template("100px 1fr fit-content(40%) auto min-content")
                .unwrap()
                .len(),
            5
        );
        assert!(template("repeat(auto-fit, minmax(120px, 1fr))").is_ok());
        for (source, token) in [
            ("[a] 1fr", "[a]"),
            ("repeat(auto-fill, 1fr)", "repeat(auto-fill, 1fr)"),
            ("minmax(1fr, 2fr)", "1fr"),
            ("calc(10px + 1fr)", "calc(10px + 1fr)"),
            ("subgrid", "subgrid"),
            ("repeat(65, 1fr)", "repeat(65, 1fr)"),
            ("10em", "10em"),
        ] {
            let error = template(source).unwrap_err();
            assert!(
                error.starts_with("UI_GRID") && error.contains(token),
                "{source}: {error}"
            );
        }
        assert!(
            template("repeat(40, 1fr) repeat(40, 1fr)")
                .unwrap_err()
                .contains("more than 64")
        );
        assert!(template("repeat(auto-fit, 10px) repeat(auto-fill, 20px)").is_err());
    }

    #[test]
    fn placements_parse_lines_spans_and_auto() {
        assert_eq!(
            placement("span 2 / span 2").unwrap().start,
            span::<GridPlacement>(2)
        );
        assert_eq!(placement("1 / -1").unwrap().end, line::<GridPlacement>(-1));
        assert_eq!(placement("3").unwrap().end, GridPlacement::Auto);
        for source in ["0", "span 0", "a / b", "1 / 2 / 3", "span 99"] {
            assert!(placement(source).is_err(), "{source}");
        }
        assert!(auto_tracks("minmax(0, 1fr)").is_ok());
        assert!(auto_tracks("repeat(2, 1fr)").is_err());
    }
}
