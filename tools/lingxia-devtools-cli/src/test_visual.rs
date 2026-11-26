//! `lxdev test --visual compare|accept`: a run's `t.visual.shot` captures
//! against per-device baselines in `<baseline dir>/<lxapp>/<device key>/<key>.png`.
//!
//! A pixel differs when a channel moves by more than [`CHANNEL_DELTA`]. A shot
//! changed when the differing area or its largest 8-connected cluster passes
//! its limit; each limit is the floor or twice what the device's own second
//! capture of the unchanged page measured, whichever is larger.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const CHANNEL_DELTA: u8 = 8;
/// 0.02% of the shot's unmasked pixels.
const AREA_FLOOR: f64 = 0.0002;
const CLUSTER_FLOOR: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum VisualMode {
    /// Compare each shot with its baseline; a changed shot fails the run
    Compare,
    /// Make each pinned shot the baseline, keeping the previous one as `.prev`
    Accept,
}

#[derive(clap::Args, Clone, Debug, Default)]
pub struct VisualArgs {
    /// Compare `t.visual.shot` captures with this device's baselines, or accept them
    #[arg(long, value_enum, help_heading = "Visual")]
    pub visual: Option<VisualMode>,
    /// Only these shots: a full key (`<spec id>/<name>`) or a shot name; repeatable
    #[arg(
        long,
        value_name = "SHOT",
        requires = "visual",
        help_heading = "Visual"
    )]
    pub only: Vec<String>,
    /// Baselines live here instead of ~/.lingxia/visual
    #[arg(long, value_name = "DIR", requires = "visual", help_heading = "Visual")]
    pub baseline_dir: Option<PathBuf>,
}

impl VisualArgs {
    pub fn control(&self) -> Option<&'static str> {
        self.visual.map(|mode| match mode {
            VisualMode::Compare => "compare",
            VisualMode::Accept => "accept",
        })
    }

    pub fn baseline_root(&self) -> Result<PathBuf> {
        if let Some(dir) = &self.baseline_dir {
            return Ok(dir.clone());
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .context("--visual needs a home directory for its baselines; pass --baseline-dir")?;
        Ok(PathBuf::from(home).join(".lingxia").join("visual"))
    }
}

// ------------------------------ pixels ------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode(bytes: &[u8]) -> Result<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => bail!("an indexed PNG was not expanded"),
    };
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

pub fn encode(image: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image.rgba)?;
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// Pixels that differ between two same-size images, outside the masks.
#[derive(Debug)]
pub struct Difference {
    pub differing: usize,
    /// Unmasked pixels compared.
    pub compared: usize,
    pub largest_cluster: usize,
    /// Per pixel: 0 same, 1 differs, 2 masked.
    map: Vec<u8>,
}

impl Difference {
    pub fn fraction(&self) -> f64 {
        if self.compared == 0 {
            0.0
        } else {
            self.differing as f64 / self.compared as f64
        }
    }
}

pub fn difference(a: &Image, b: &Image, masks: &[Rect]) -> Result<Difference, String> {
    if (a.width, a.height) != (b.width, b.height) {
        return Err(format!(
            "the shot is {}x{} but its baseline is {}x{}; a different screen size or fold state needs its own baseline",
            b.width, b.height, a.width, a.height
        ));
    }
    let (w, h) = (a.width as usize, a.height as usize);
    let mut map = vec![0u8; w * h];
    for mask in masks {
        let x0 = mask.x.clamp(0, w as i64) as usize;
        let y0 = mask.y.clamp(0, h as i64) as usize;
        let x1 = (mask.x + mask.width).clamp(0, w as i64) as usize;
        let y1 = (mask.y + mask.height).clamp(0, h as i64) as usize;
        for y in y0..y1 {
            map[y * w + x0..y * w + x1].fill(2);
        }
    }
    let mut differing = 0;
    let mut compared = 0;
    for (i, cell) in map.iter_mut().enumerate() {
        if *cell == 2 {
            continue;
        }
        compared += 1;
        let (pa, pb) = (&a.rgba[i * 4..i * 4 + 4], &b.rgba[i * 4..i * 4 + 4]);
        if pa
            .iter()
            .zip(pb)
            .any(|(x, y)| x.abs_diff(*y) > CHANNEL_DELTA)
        {
            *cell = 1;
            differing += 1;
        }
    }
    let largest_cluster = largest_cluster(&map, w, h);
    Ok(Difference {
        differing,
        compared,
        largest_cluster,
        map,
    })
}

fn largest_cluster(map: &[u8], w: usize, h: usize) -> usize {
    let mut seen = vec![false; map.len()];
    let mut stack = Vec::new();
    let mut largest = 0;
    for start in 0..map.len() {
        if map[start] != 1 || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut size = 0;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = ((i % w) as i64, (i / w) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let n = ny as usize * w + nx as usize;
                    if map[n] == 1 && !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        largest = largest.max(size);
    }
    largest
}

/// The shot faded, its differing pixels red and its masks grey-blue.
fn diff_image(after: &Image, difference: &Difference) -> Image {
    let mut rgba = Vec::with_capacity(after.rgba.len());
    for (i, cell) in difference.map.iter().enumerate() {
        let p = &after.rgba[i * 4..i * 4 + 4];
        let luma = (p[0] as u32 * 3 + p[1] as u32 * 6 + p[2] as u32) / 10;
        let faded = (luma * 3 / 10 + 255 * 7 / 10) as u8;
        rgba.extend_from_slice(&match cell {
            1 => [230, 0, 40, 255],
            2 => [faded / 2 + 50, faded / 2 + 60, faded / 2 + 100, 255],
            _ => [faded, faded, faded, 255],
        });
    }
    Image {
        width: after.width,
        height: after.height,
        rgba,
    }
}

// ------------------------------ shots ------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Same,
    Changed,
    New,
    SizeMismatch(String),
    Accepted { replaced: bool },
    Refused(String),
    Unreadable(String),
}

impl Status {
    fn label(&self) -> &'static str {
        match self {
            Status::Same => "same",
            Status::Changed => "changed",
            Status::New => "new",
            Status::SizeMismatch(_) => "size mismatch",
            Status::Accepted { .. } => "accepted",
            Status::Refused(_) => "refused",
            Status::Unreadable(_) => "unreadable",
        }
    }

    fn fails(&self) -> bool {
        matches!(
            self,
            Status::Changed | Status::SizeMismatch(_) | Status::Unreadable(_)
        )
    }

    /// Report order: what needs a look first.
    fn rank(&self) -> u8 {
        match self {
            Status::Changed | Status::SizeMismatch(_) | Status::Unreadable(_) => 0,
            Status::Refused(_) => 1,
            Status::New => 2,
            Status::Accepted { .. } => 3,
            Status::Same => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Measure {
    pub differing: usize,
    pub fraction: f64,
    pub largest_cluster: usize,
}

impl Measure {
    fn of(difference: &Difference) -> Self {
        Self {
            differing: difference.differing,
            fraction: difference.fraction(),
            largest_cluster: difference.largest_cluster,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShotResult {
    pub key: String,
    pub device_key: String,
    /// The device as a person names it, key included; shown, never matched.
    pub device_label: String,
    pub status: Status,
    pub pinned: bool,
    pub settled: Value,
    pub change: Option<Measure>,
    pub noise: Option<Measure>,
    pub area_limit: f64,
    pub cluster_limit: usize,
    /// Report-relative images.
    pub before: Option<String>,
    pub after: String,
    pub diff: Option<String>,
    /// What changed under the shot since its baseline: OS build, text scale, language.
    pub context: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Summary {
    pub shots: Vec<ShotResult>,
    /// Shots taken without a pinned appearance.
    pub unpinned: usize,
}

impl Summary {
    pub fn failures(&self) -> Vec<String> {
        self.shots
            .iter()
            .filter(|shot| shot.status.fails())
            .map(|shot| match &shot.status {
                Status::SizeMismatch(why) | Status::Unreadable(why) => {
                    format!("visual {}: {why}", shown_key(&shot.key))
                }
                _ => format!(
                    "visual {}: changed against its baseline",
                    shown_key(&shot.key)
                ),
            })
            .collect()
    }
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Platform, maker and model. A shot of another size on the same device (a
/// fold, a rotation) fails as a size mismatch rather than starting a baseline.
pub fn device_key(sidecar: &Value, target: &str) -> String {
    let device = &sidecar["device"];
    let part = |field: &str| device[field].as_str().filter(|s| !s.is_empty()).map(slug);
    let named = [
        part("osName").unwrap_or_else(|| slug(target)),
        part("brand").unwrap_or_default(),
        part("model").unwrap_or_else(|| "unknown".into()),
    ];
    named
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("-")
}

/// The marketing name beside the key, when the device reports one other than its model id.
pub fn device_label(sidecar: &Value, key: &str) -> String {
    let device = &sidecar["device"];
    let model = device["model"].as_str().unwrap_or_default();
    match device["marketName"].as_str().map(str::trim) {
        Some(name) if !name.is_empty() && !name.eq_ignore_ascii_case(model) => {
            format!("{name} ({key})")
        }
        _ => key.to_owned(),
    }
}

fn selected(only: &[String], key: &str) -> bool {
    only.is_empty()
        || only
            .iter()
            .any(|want| key == want || key.rsplit('/').next() == Some(want.as_str()))
}

fn read_image(path: &Path) -> Result<Image> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    decode(&bytes).with_context(|| format!("{} is not a readable PNG", path.display()))
}

/// The sidecars among a run's artifacts.
pub fn sidecars(artifacts: &[(String, PathBuf, usize)]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = artifacts
        .iter()
        .filter(|(name, _, _)| {
            name.starts_with("attachments/") && name.contains("/visual/") && name.ends_with(".json")
        })
        .map(|(_, path, _)| path.clone())
        .collect();
    found.sort();
    found
}

pub struct Run<'a> {
    pub output_dir: &'a Path,
    pub target: &'a str,
    pub args: &'a VisualArgs,
    pub run_id: &'a str,
}

/// Compare or accept every selected shot; writes before/diff images under `visual/`.
pub fn process(run: &Run, sidecars: &[PathBuf]) -> Result<Summary> {
    let mode = run.args.visual.context("no --visual mode")?;
    let root = run.args.baseline_root()?;
    let mut summary = Summary::default();
    for path in sidecars {
        let sidecar: Value = serde_json::from_slice(&std::fs::read(path)?)
            .with_context(|| format!("{} is not a shot sidecar", path.display()))?;
        let Some(key) = sidecar["key"].as_str().map(str::to_owned) else {
            continue;
        };
        if !selected(&run.args.only, &key) {
            continue;
        }
        let pinned = sidecar["environment"]["pinned"] == true;
        if !pinned {
            summary.unpinned += 1;
        }
        let after_rel = sidecar["png"].as_str().unwrap_or_default().to_owned();
        let mut result = ShotResult {
            key: key.clone(),
            device_key: String::new(),
            device_label: String::new(),
            status: Status::Same,
            pinned,
            settled: sidecar["settled"].clone(),
            change: None,
            noise: None,
            area_limit: AREA_FLOOR,
            cluster_limit: CLUSTER_FLOOR,
            before: None,
            after: after_rel.clone(),
            diff: None,
            context: Vec::new(),
        };
        let after = match read_image(&run.output_dir.join(&after_rel)) {
            Ok(image) => image,
            Err(error) => {
                result.status = Status::Unreadable(format!("{error:#}"));
                summary.shots.push(result);
                continue;
            }
        };
        let masks: Vec<Rect> = serde_json::from_value(sidecar["masks"].clone()).unwrap_or_default();
        result.device_key = device_key(&sidecar, run.target);
        result.device_label = device_label(&sidecar, &result.device_key);
        if let Some(twin) = sidecar["noise"].as_str()
            && let Ok(twin) = read_image(&run.output_dir.join(twin))
            && let Ok(noise) = difference(&after, &twin, &masks)
        {
            let noise = Measure::of(&noise);
            result.area_limit = AREA_FLOOR.max(2.0 * noise.fraction);
            result.cluster_limit = CLUSTER_FLOOR.max(2 * noise.largest_cluster);
            result.noise = Some(noise);
        }
        let app = sidecar["app"]
            .as_str()
            .map(slug)
            .unwrap_or_else(|| "unknown-app".into());
        let device_dir = root.join(&app).join(&result.device_key);
        let size = format!("{}x{}", after.width, after.height);
        let baseline = device_dir.join(&size).join(format!("{key}.png"));
        let shot_dir = Path::new("visual").join(&key);
        let elsewhere = if baseline.exists() {
            Vec::new()
        } else {
            other_sizes(&device_dir, &size, &key)
        };
        if let Some((first, _)) = elsewhere.first() {
            let before_rel = shot_dir.join("before.png");
            write(run.output_dir, &before_rel, &std::fs::read(first)?)?;
            result.before = Some(slashed(&before_rel));
            let sizes: Vec<&str> = elsewhere.iter().map(|(_, size)| size.as_str()).collect();
            result.status = Status::SizeMismatch(format!(
                "no baseline at {size}; {} at {} — a different screen size or fold state needs its own baseline, or the layout changed size",
                if sizes.len() == 1 {
                    "one exists"
                } else {
                    "baselines exist"
                },
                sizes.join(", ")
            ));
        } else if baseline.exists() {
            result.context = context(&baseline.with_extension("json"), &sidecar);
            let before = read_image(&baseline)?;
            let before_rel = shot_dir.join("before.png");
            write(run.output_dir, &before_rel, &std::fs::read(&baseline)?)?;
            result.before = Some(slashed(&before_rel));
            match difference(&before, &after, &masks) {
                Err(why) => result.status = Status::SizeMismatch(why),
                Ok(change) => {
                    let measure = Measure::of(&change);
                    let changed = measure.fraction > result.area_limit
                        || measure.largest_cluster > result.cluster_limit;
                    result.status = if changed {
                        Status::Changed
                    } else {
                        Status::Same
                    };
                    if measure.differing > 0 || !masks.is_empty() {
                        let diff_rel = shot_dir.join("diff.png");
                        write(
                            run.output_dir,
                            &diff_rel,
                            &encode(&diff_image(&after, &change))?,
                        )?;
                        result.diff = Some(slashed(&diff_rel));
                    }
                    result.change = Some(measure);
                }
            }
        } else {
            result.status = Status::New;
        }
        if mode == VisualMode::Accept {
            // Only a baseline at this size is replaced; one at another size stays.
            let replaced = baseline.exists();
            result.status = if pinned {
                accept(
                    &baseline,
                    &run.output_dir.join(&after_rel),
                    &sidecar,
                    &result,
                    run.run_id,
                )?;
                Status::Accepted { replaced }
            } else {
                Status::Refused(
                    "the shot's appearance was not pinned; call t.app.view.environment({ appearance }) before t.visual.shot".into(),
                )
            };
        }
        summary.shots.push(result);
    }
    summary.shots.sort_by(|a, b| {
        a.status
            .rank()
            .cmp(&b.status.rank())
            .then_with(|| a.key.cmp(&b.key))
    });
    Ok(summary)
}

/// Baselines of `key` at other screen sizes of the device, with their size.
fn other_sizes(device_dir: &Path, size: &str, key: &str) -> Vec<(PathBuf, String)> {
    let mut found: Vec<(PathBuf, String)> = std::fs::read_dir(device_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let other = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path().join(format!("{key}.png"));
            (other != size && path.exists()).then_some((path, other))
        })
        .collect();
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

/// What moves every pixel without being a regression, named when it differs from the baseline's.
fn context(record: &Path, sidecar: &Value) -> Vec<String> {
    let Ok(record) = std::fs::read(record)
        .map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap_or_default())
    else {
        return Vec::new();
    };
    let shown = |value: &Value| match value {
        Value::Null => "unknown".to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let mut changed = Vec::new();
    let os = &sidecar["device"]["osVersion"];
    if record["osVersion"] != *os {
        changed.push(format!(
            "OS {} → {}",
            shown(&record["osVersion"]),
            shown(os)
        ));
    }
    if record["textScale"] != sidecar["textScale"] {
        changed.push("text scale changed".to_string());
    }
    if record["language"] != sidecar["language"] {
        changed.push(format!(
            "language {} → {}",
            shown(&record["language"]),
            shown(&sidecar["language"])
        ));
    }
    changed
}

/// Install the shot as the baseline; the one it replaces becomes `<key>.prev.png`.
fn accept(
    baseline: &Path,
    shot: &Path,
    sidecar: &Value,
    result: &ShotResult,
    run_id: &str,
) -> Result<()> {
    let dir = baseline
        .parent()
        .context("baseline path has no directory")?;
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let with_ext = |ext: &str| {
        let name = baseline.file_name().unwrap_or_default().to_string_lossy();
        baseline.with_file_name(format!("{}{ext}", name.trim_end_matches(".png")))
    };
    if baseline.exists() {
        std::fs::rename(baseline, with_ext(".prev.png"))?;
        let json = with_ext(".json");
        if json.exists() {
            std::fs::rename(&json, with_ext(".prev.json"))?;
        }
    }
    std::fs::copy(shot, baseline)
        .with_context(|| format!("failed to write {}", baseline.display()))?;
    let record = json!({
        "key": result.key,
        "run": run_id,
        "acceptedAt": chrono::Utc::now().to_rfc3339(),
        "environment": sidecar["environment"],
        "device": sidecar["device"],
        "settled": sidecar["settled"],
        "masks": sidecar["masks"],
        "osVersion": sidecar["device"]["osVersion"],
        "textScale": sidecar["textScale"],
        "language": sidecar["language"],
        "noise": result.noise.map(|n| json!({ "fraction": n.fraction, "largestCluster": n.largest_cluster })),
    });
    std::fs::write(with_ext(".json"), serde_json::to_vec_pretty(&record)?)?;
    Ok(())
}

fn write(output_dir: &Path, rel: &Path, bytes: &[u8]) -> Result<()> {
    let path = output_dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn slashed(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

// ------------------------------ report ------------------------------

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A report-relative path as an `src`: `%` in encoded spec ids must survive.
fn src(path: &str) -> String {
    escape(
        &path
            .replace('%', "%25")
            .replace('#', "%23")
            .replace('?', "%3F"),
    )
}

fn percent(fraction: f64) -> String {
    format!("{:.3}%", fraction * 100.0)
}

/// The report section: one row per shot, changed first; before | after | diff.
pub fn section(summary: &Summary, mode: VisualMode) -> String {
    let mut html = String::new();
    let count = |label: &str| {
        summary
            .shots
            .iter()
            .filter(|s| s.status.label() == label)
            .count()
    };
    let _ = write!(
        html,
        "<section id=\"visual\" class=\"lx-visual\"><h2>Visual {}</h2><p>{} shot{}: {} changed, {} size mismatch, {} new, {} same",
        if mode == VisualMode::Accept {
            "accept"
        } else {
            "comparison"
        },
        summary.shots.len(),
        if summary.shots.len() == 1 { "" } else { "s" },
        count("changed") + count("unreadable"),
        count("size mismatch"),
        count("new"),
        count("same"),
    );
    if count("accepted") + count("refused") > 0 {
        let _ = write!(
            html,
            ", {} accepted, {} refused",
            count("accepted"),
            count("refused")
        );
    }
    html.push_str(".</p>");
    if summary.unpinned > 0 {
        let _ = write!(
            html,
            "<p class=\"lx-visual-warn\">{} without a pinned appearance and cannot become {}.</p>",
            shots_were(summary.unpinned),
            if summary.unpinned == 1 {
                "a baseline"
            } else {
                "baselines"
            }
        );
    }
    let mut folded = false;
    for shot in &summary.shots {
        // Unchanged shots fold away below the ones that need a look.
        if matches!(shot.status, Status::Same | Status::Accepted { .. }) && !folded {
            folded = true;
            let quiet = summary
                .shots
                .iter()
                .filter(|s| matches!(s.status, Status::Same | Status::Accepted { .. }))
                .count();
            let _ = write!(
                html,
                "<details class=\"lx-visual-quiet\"><summary>{quiet} unchanged or accepted</summary>"
            );
        }
        let detail = match &shot.status {
            Status::SizeMismatch(why) | Status::Refused(why) | Status::Unreadable(why) => {
                escape(why)
            }
            Status::New => "no baseline for this device yet".into(),
            _ => match shot.change {
                Some(m) => format!(
                    "{} px differ ({}), largest cluster {} px; limits {} and {} px",
                    m.differing,
                    percent(m.fraction),
                    m.largest_cluster,
                    percent(shot.area_limit),
                    shot.cluster_limit
                ),
                None => String::new(),
            },
        };
        let noise = match shot.noise {
            Some(n) => format!(
                "device noise {} px ({}), cluster {} px",
                n.differing,
                percent(n.fraction),
                n.largest_cluster
            ),
            None => "device noise not measured".into(),
        };
        let mut notes = vec![noise, format!("device {}", escape(&shot.device_label))];
        notes.extend(shot.context.iter().map(|note| escape(note)));
        if !shot.pinned {
            notes.push("appearance not pinned".into());
        }
        if shot.settled == false {
            notes.push("not settled".into());
        }
        let image = |label: &str, path: &Option<String>| match path {
            Some(path) => format!(
                "<figure><figcaption>{label}</figcaption><a href=\"{0}\"><img src=\"{0}\" alt=\"{label}\" loading=\"lazy\"></a></figure>",
                src(path)
            ),
            None => format!(
                "<figure><figcaption>{label}</figcaption><div class=\"lx-visual-none\">none</div></figure>"
            ),
        };
        let _ = write!(
            html,
            "<article class=\"lx-visual-shot lx-visual-{}\"><h3><span class=\"lx-visual-status\">{}</span> {}</h3><p>{}</p><p class=\"lx-visual-notes\">{}</p><div class=\"lx-visual-row\">{}{}{}</div></article>",
            shot.status.label().replace(' ', "-"),
            shot.status.label(),
            escape(&shown_key(&shot.key)),
            detail,
            notes.join(" · "),
            image("before", &shot.before),
            image("after", &Some(shot.after.clone())),
            image("diff", &shot.diff),
        );
    }
    if folded {
        html.push_str("</details>");
    }
    html.push_str("</section>");
    html
}

fn shots_were(count: usize) -> String {
    if count == 1 {
        "1 shot was taken".into()
    } else {
        format!("{count} shots were taken")
    }
}

/// The key as its author wrote it: spec ids are percent-encoded on disk.
fn shown_key(key: &str) -> String {
    let bytes = key.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = key.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| key.to_string())
}

const STYLE: &str = "<style>
.lx-visual{font:14px/1.45 system-ui,sans-serif;color:#1d2330;background:#f4f5f8;border-radius:10px;padding:16px;margin:24px 0}
.lx-visual-quiet summary{cursor:pointer;color:#5b6475;margin:12px 0}
.lx-visual h2{font-size:18px;margin:0 0 8px}
.lx-visual-warn{color:#8a5a00}
.lx-visual-shot{border:1px solid #d9dde5;border-radius:8px;padding:12px 14px;margin:12px 0;background:#fff}
.lx-visual-shot h3{font-size:15px;margin:0 0 4px;word-break:break-all}
.lx-visual-shot p{margin:2px 0}
.lx-visual-notes{color:#5b6475;font-size:12px}
.lx-visual-status{display:inline-block;font-size:12px;padding:1px 8px;border-radius:10px;background:#e6e9ef;margin-right:6px}
.lx-visual-changed .lx-visual-status,.lx-visual-size-mismatch .lx-visual-status,.lx-visual-unreadable .lx-visual-status{background:#fde2e4;color:#a1122b}
.lx-visual-new .lx-visual-status,.lx-visual-refused .lx-visual-status{background:#fff1d6;color:#8a5a00}
.lx-visual-same .lx-visual-status,.lx-visual-accepted .lx-visual-status{background:#dff5e6;color:#17663a}
.lx-visual-row{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px;margin-top:8px}
.lx-visual figure{margin:0}
.lx-visual figcaption{font-size:12px;color:#5b6475;margin-bottom:4px}
.lx-visual img{display:block;max-width:100%;max-height:560px;margin:0 auto;border:1px solid #d9dde5;border-radius:4px;background:#f4f5f8}
.lx-visual-none{border:1px dashed #c9ced8;border-radius:4px;padding:24px 0;text-align:center;color:#8a93a3;font-size:12px}
@media (max-width:640px){.lx-visual-row{grid-template-columns:1fr}}
@media (prefers-color-scheme:dark){.lx-visual{color:#e3e7ee;background:#12151b}.lx-visual-warn{color:#e6b450}.lx-visual-quiet summary{color:#9aa3b2}.lx-visual-shot{background:#1b1f27;border-color:#323846}.lx-visual-notes,.lx-visual figcaption{color:#9aa3b2}.lx-visual-status{background:#2b313d}.lx-visual-changed .lx-visual-status,.lx-visual-size-mismatch .lx-visual-status,.lx-visual-unreadable .lx-visual-status{background:#3d1c22;color:#ff8a9a}.lx-visual-new .lx-visual-status,.lx-visual-refused .lx-visual-status{background:#3a2e14;color:#f0c060}.lx-visual-same .lx-visual-status,.lx-visual-accepted .lx-visual-status{background:#173322;color:#7ad49a}.lx-visual img{border-color:#323846;background:#252a34}}
</style>";

/// `visual.html` beside the reports, and the same section inside report.html.
pub fn write_report(output_dir: &Path, summary: &Summary, mode: VisualMode) -> Result<()> {
    let section = section(summary, mode);
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Visual comparison</title>{STYLE}</head><body style=\"margin:0 auto;max-width:1200px;padding:0 16px;background:Canvas;color:CanvasText;color-scheme:light dark\">{section}</body></html>"
    );
    std::fs::write(output_dir.join("visual.html"), page)?;
    let report = output_dir.join("report.html");
    if let Ok(html) = std::fs::read_to_string(&report) {
        let insert = format!("{STYLE}{section}");
        let html = match html.rfind("</body>") {
            Some(at) => format!("{}{insert}{}", &html[..at], &html[at..]),
            None => format!("{html}{insert}"),
        };
        std::fs::write(&report, html)?;
    }
    Ok(())
}

/// One line per shot that needs a look, then the totals.
pub fn print(summary: &Summary, mode: VisualMode, output_dir: &Path) {
    use owo_colors::OwoColorize;
    if summary.unpinned > 0 && mode == VisualMode::Compare {
        eprintln!(
            "{} {} without a pinned appearance; `--visual accept` will refuse {} (pin with t.app.view.environment)",
            "visual".yellow(),
            shots_were(summary.unpinned),
            if summary.unpinned == 1 { "it" } else { "them" }
        );
    }
    for shot in &summary.shots {
        let key = shown_key(&shot.key);
        let line = match &shot.status {
            Status::Same => continue,
            Status::Changed => format!("{} {}", "changed".red(), key),
            Status::SizeMismatch(why) | Status::Unreadable(why) => {
                format!("{} {}: {why}", "failed".red(), key)
            }
            Status::New => format!(
                "{} {} (no baseline for {})",
                "new".yellow(),
                key,
                shot.device_label
            ),
            Status::Refused(why) => format!("{} {}: {why}", "refused".yellow(), key),
            Status::Accepted { replaced } => format!(
                "{} {} for {}{}",
                "accepted".green(),
                key,
                shot.device_label,
                if *replaced {
                    " (previous kept as .prev)"
                } else {
                    ""
                }
            ),
        };
        eprintln!("{} {line}", "visual".cyan());
    }
    let worst_noise = summary
        .shots
        .iter()
        .filter_map(|s| s.noise)
        .map(|n| n.fraction)
        .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
    eprintln!(
        "{} {} shot{}{}; {}",
        "visual".cyan(),
        summary.shots.len(),
        if summary.shots.len() == 1 { "" } else { "s" },
        worst_noise
            .map(|n| format!(", worst device noise {}", percent(n)))
            .unwrap_or_default(),
        output_dir.join("visual.html").display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 4]) -> Image {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&paint(x, y));
            }
        }
        Image {
            width,
            height,
            rgba,
        }
    }

    /// A card with a 1 px border, the kind of edge a regression moves.
    fn card(border_x: u32) -> Image {
        image(200, 300, move |x, y| {
            if x == border_x && (40..260).contains(&y) {
                [40, 40, 60, 255]
            } else {
                [250, 250, 252, 255]
            }
        })
    }

    /// Anti-aliasing jitter: every pixel nudged under the channel threshold, and
    /// a few isolated pixels over it.
    fn jittered(base: &Image) -> Image {
        let mut out = base.clone();
        for (i, px) in out.rgba.chunks_mut(4).enumerate() {
            let nudge = (i * 7919 % 13) as u8;
            px[0] = px[0].saturating_sub(nudge.min(6));
            if i % 9973 == 0 {
                px[1] = px[1].saturating_sub(40);
            }
        }
        out
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        output: PathBuf,
        baselines: PathBuf,
        sidecars: Vec<PathBuf>,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let output = dir.path().join("run");
            let baselines = dir.path().join("baselines");
            std::fs::create_dir_all(&output).unwrap();
            Self {
                _dir: dir,
                output,
                baselines,
                sidecars: Vec::new(),
            }
        }

        fn shot(
            &mut self,
            name: &str,
            shot: &Image,
            twin: Option<&Image>,
            masks: Value,
            pinned: bool,
        ) {
            let dir = "attachments/spec%2Fone/attempt-0/visual".to_string();
            let png = format!("{dir}/{name}.png");
            write(&self.output, Path::new(&png), &encode(shot).unwrap()).unwrap();
            let noise = twin.map(|twin| {
                let path = format!("{dir}/{name}.noise.png");
                write(&self.output, Path::new(&path), &encode(twin).unwrap()).unwrap();
                path
            });
            let sidecar = json!({
                "key": format!("spec%2Fone/{name}"),
                "app": "app.lingxia.gallery",
                "device": { "brand": "Xiaomi", "model": "Redmi 9", "osName": "Android", "osVersion": "11" },
                "png": png, "noise": noise, "width": shot.width, "height": shot.height,
                "settled": true,
                "environment": { "pinned": pinned, "appearance": if pinned { json!("dark") } else { Value::Null }, "still": pinned },
                "masks": masks,
                "textScale": null,
                "language": "en-US",
            });
            let path = self.output.join(format!("{dir}/{name}.json"));
            std::fs::write(&path, serde_json::to_vec(&sidecar).unwrap()).unwrap();
            self.sidecars.push(path);
        }

        fn baseline(&self, name: &str, image: &Image) {
            let path = self
                .baselines
                .join("app.lingxia.gallery/android-xiaomi-redmi-9")
                .join(format!("{}x{}", image.width, image.height))
                .join(format!("spec%2Fone/{name}.png"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, encode(image).unwrap()).unwrap();
            let record = json!({ "osVersion": "10", "textScale": null, "language": "en-US" });
            std::fs::write(path.with_extension("json"), record.to_string()).unwrap();
        }

        fn run(&self, mode: VisualMode, only: &[&str]) -> Summary {
            let args = VisualArgs {
                visual: Some(mode),
                only: only.iter().map(|s| s.to_string()).collect(),
                baseline_dir: Some(self.baselines.clone()),
            };
            let run = Run {
                output_dir: &self.output,
                target: "android",
                args: &args,
                run_id: "run-1",
            };
            let summary = process(&run, &self.sidecars).unwrap();
            write_report(&self.output, &summary, mode).unwrap();
            summary
        }
    }

    fn status<'a>(summary: &'a Summary, name: &str) -> &'a ShotResult {
        summary
            .shots
            .iter()
            .find(|s| s.key.ends_with(&format!("/{name}")))
            .unwrap()
    }

    #[test]
    fn png_round_trips_and_device_keys_name_the_device() {
        let card = card(20);
        assert_eq!(decode(&encode(&card).unwrap()).unwrap(), card);
        let sidecar =
            json!({ "device": { "brand": "HUAWEI", "model": "ALT-AL10", "osName": "Harmony" } });
        assert_eq!(device_key(&sidecar, "harmony"), "harmony-huawei-alt-al10");
        assert_eq!(device_key(&json!({ "device": null }), "ios"), "ios-unknown");
        // The marketing name is shown beside the key and never changes it.
        let named = json!({ "device": { "brand": "HUAWEI", "model": "ALT-AL10", "marketName": "HUAWEI Mate X5", "osName": "Harmony" } });
        assert_eq!(device_key(&named, "harmony"), "harmony-huawei-alt-al10");
        assert_eq!(
            device_label(&named, "harmony-huawei-alt-al10"),
            "HUAWEI Mate X5 (harmony-huawei-alt-al10)"
        );
        assert_eq!(
            device_label(&sidecar, "harmony-huawei-alt-al10"),
            "harmony-huawei-alt-al10"
        );
    }

    #[test]
    fn compare_grades_identical_noise_moved_masked_resized_and_new_shots() {
        let mut fixture = Fixture::new();
        let base = card(20);
        for name in ["identical", "noise", "moved", "masked", "resized"] {
            fixture.baseline(name, &base);
        }
        fixture.shot("identical", &base, Some(&base), json!([]), true);
        fixture.shot(
            "noise",
            &jittered(&base),
            Some(&jittered(&jittered(&base))),
            json!([]),
            true,
        );
        fixture.shot("moved", &card(23), Some(&card(23)), json!([]), true);
        let blotted = image(200, 300, |x, y| {
            if (100..180).contains(&x) && (100..200).contains(&y) {
                [0, 0, 0, 255]
            } else {
                base.rgba[((y * 200 + x) * 4) as usize..][..4]
                    .try_into()
                    .unwrap()
            }
        });
        fixture.shot(
            "masked",
            &blotted,
            None,
            json!([{ "x": 100, "y": 100, "width": 80, "height": 100, "from": "rect" }]),
            true,
        );
        fixture.shot(
            "resized",
            &image(200, 280, |_, _| [250, 250, 252, 255]),
            None,
            json!([]),
            true,
        );
        fixture.shot("fresh", &base, None, json!([]), false);

        std::fs::write(
            fixture.output.join("report.html"),
            "<html><body><h1>Run</h1></body></html>",
        )
        .unwrap();
        let summary = fixture.run(VisualMode::Compare, &[]);
        let report = std::fs::read_to_string(fixture.output.join("report.html")).unwrap();
        assert!(report.find("<h1>Run</h1>").unwrap() < report.find("id=\"visual\"").unwrap());
        assert!(report.ends_with("</section></body></html>"));
        assert_eq!(status(&summary, "identical").status, Status::Same);
        let noise = status(&summary, "noise");
        assert_eq!(noise.status, Status::Same, "{noise:?}");
        assert!(
            noise.change.unwrap().differing > 0,
            "the isolated pixels are seen, and stay under the limits"
        );
        let moved = status(&summary, "moved");
        assert_eq!(moved.status, Status::Changed);
        assert!(moved.change.unwrap().largest_cluster > CLUSTER_FLOOR);
        assert_eq!(status(&summary, "masked").status, Status::Same);
        match &status(&summary, "resized").status {
            Status::SizeMismatch(why) => assert!(
                why.starts_with("no baseline at 200x280; one exists at 200x300 — "),
                "{why}"
            ),
            other => panic!("{other:?}"),
        }
        assert_eq!(status(&summary, "fresh").status, Status::New);
        assert_eq!(summary.unpinned, 1);
        // The baselines were taken on an older OS build: every compared card says so.
        assert_eq!(status(&summary, "moved").context, ["OS 10 → 11"]);
        // Changed first; a failure names the shot.
        assert!(summary.shots[0].status.fails() && summary.shots[1].status.fails());
        let failures = summary.failures();
        assert_eq!(failures.len(), 2);
        assert!(failures.iter().any(|f| f.contains("spec/one/moved")));

        let html = std::fs::read_to_string(fixture.output.join("visual.html")).unwrap();
        assert!(
            html.find(" spec/one/moved</h3>").unwrap()
                < html.find(" spec/one/identical</h3>").unwrap()
        );
        assert!(
            html.contains("visual/spec%252Fone/moved/diff.png"),
            "an encoded spec id stays a working src"
        );
        assert!(
            fixture
                .output
                .join("visual/spec%2Fone/moved/diff.png")
                .exists()
        );
        assert!(
            fixture
                .output
                .join("visual/spec%2Fone/moved/before.png")
                .exists()
        );
        if let Some(dir) = std::env::var_os("LXDEV_VISUAL_SAMPLE_DIR") {
            copy_tree(&fixture.output, Path::new(&dir));
        }
    }

    #[test]
    fn device_noise_raises_the_limits() {
        let mut fixture = Fixture::new();
        let base = card(20);
        fixture.baseline("busy", &base);
        // The device itself flickers a 40 px strip between captures, so the same strip against the baseline is noise.
        let flicker = |size: u32| {
            image(200, 300, move |x, y| {
                if x < size && y < 2 {
                    [0, 0, 0, 255]
                } else {
                    base_px(x, y)
                }
            })
        };
        fixture.shot("busy", &flicker(20), Some(&flicker(0)), json!([]), true);
        let shot = &fixture.run(VisualMode::Compare, &[]).shots[0];
        assert_eq!(shot.cluster_limit, 80);
        assert_eq!(shot.status, Status::Same, "{shot:?}");
    }

    fn base_px(x: u32, y: u32) -> [u8; 4] {
        if x == 20 && (40..260).contains(&y) {
            [40, 40, 60, 255]
        } else {
            [250, 250, 252, 255]
        }
    }

    #[test]
    fn accept_installs_pinned_shots_keeps_one_previous_and_refuses_unpinned() {
        let mut fixture = Fixture::new();
        fixture.baseline("home", &card(20));
        fixture.shot("home", &card(23), None, json!([]), true);
        fixture.shot("loose", &card(23), None, json!([]), false);
        fixture.shot("other", &card(23), None, json!([]), true);

        let summary = fixture.run(VisualMode::Accept, &["home", "spec%2Fone/loose"]);
        assert_eq!(summary.shots.len(), 2, "--only leaves `other` alone");
        assert_eq!(
            status(&summary, "home").status,
            Status::Accepted { replaced: true }
        );
        assert!(matches!(
            status(&summary, "loose").status,
            Status::Refused(_)
        ));
        assert!(summary.failures().is_empty());
        let dir = fixture
            .baselines
            .join("app.lingxia.gallery/android-xiaomi-redmi-9/200x300/spec%2Fone");
        assert_eq!(read_image(&dir.join("home.png")).unwrap(), card(23));
        assert_eq!(read_image(&dir.join("home.prev.png")).unwrap(), card(20));
        let record: Value =
            serde_json::from_slice(&std::fs::read(dir.join("home.json")).unwrap()).unwrap();
        assert_eq!(record["environment"]["appearance"], "dark");
        assert_eq!(
            (&record["osVersion"], &record["language"]),
            (&json!("11"), &json!("en-US"))
        );
        assert!(!dir.join("loose.png").exists() && !dir.join("other.png").exists());

        // A second accept keeps only one previous generation.
        fixture.run(VisualMode::Accept, &["home"]);
        assert_eq!(read_image(&dir.join("home.prev.png")).unwrap(), card(23));
        assert!(!dir.join("home.prev.prev.png").exists());
    }

    #[test]
    fn a_fold_gets_its_own_baseline_once_accepted() {
        let mut fixture = Fixture::new();
        fixture.baseline("home", &card(20));
        let folded = image(200, 280, |_, _| [250, 250, 252, 255]);
        fixture.shot("home", &folded, None, json!([]), true);
        assert!(matches!(
            fixture.run(VisualMode::Compare, &[]).shots[0].status,
            Status::SizeMismatch(_)
        ));
        assert_eq!(
            fixture.run(VisualMode::Accept, &[]).shots[0].status,
            Status::Accepted { replaced: false }
        );
        let device = fixture
            .baselines
            .join("app.lingxia.gallery/android-xiaomi-redmi-9");
        assert_eq!(
            read_image(&device.join("200x280/spec%2Fone/home.png")).unwrap(),
            folded
        );
        assert_eq!(
            read_image(&device.join("200x300/spec%2Fone/home.png")).unwrap(),
            card(20)
        );
        assert_eq!(
            fixture.run(VisualMode::Compare, &[]).shots[0].status,
            Status::Same
        );
    }

    fn copy_tree(from: &Path, to: &Path) {
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                std::fs::create_dir_all(&target).unwrap();
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::create_dir_all(to).unwrap();
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
}
