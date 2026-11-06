//! Renders compiled View pages headlessly for the corpus: deterministic cost counters per
//! page and viewport, informative timings, optional PNGs, and a hash of the CPU-rendered frame.
//! Driven by packages/lingxia-view/corpus/run.mjs.
//!   cargo run -p lingxia-view --example corpus -- jobs.json results.json

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Job {
    id: String,
    module: String,
    width: u16,
    height: u16,
    scale: f32,
    dark: bool,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    state: Option<Value>,
    #[serde(default)]
    png: Option<String>,
    /// Writes the frame's node snapshots, as adapters receive them.
    #[serde(default)]
    nodes: Option<String>,
    /// Directory an `<image src>` resolves against.
    #[serde(default)]
    root: Option<String>,
    /// Rasterizes the frame and reports its hash.
    #[serde(default)]
    hash: bool,
}

/// The frame's identity: SHA-256 over its size and RGBA bytes.
fn frame_hash(frame: &vello_cpu::Pixmap) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(frame.width().to_be_bytes());
    hasher.update(frame.height().to_be_bytes());
    hasher.update(frame.data_as_u8_slice());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn session(job: &Job, source: &str) -> Result<lingxia_view::ViewSession, String> {
    let mut page = lingxia_view::ViewSession::new(source)?;
    let facts = lingxia_view::platform::DeviceFacts {
        width: (f32::from(job.width) * job.scale) as u32,
        height: (f32::from(job.height) * job.scale) as u32,
        scale: job.scale,
        os: "corpus".into(),
        dark: job.dark,
        smallest_width: f32::from(job.width.min(job.height)),
        locale: job.locale.clone().unwrap_or_else(|| "en".into()),
        animation_scale: 1.0,
        // `__pointer: "fine"` renders the desktop look (popovers, hover).
        fine_pointer: job.state.as_ref().and_then(|s| s["__pointer"].as_str()) == Some("fine"),
        ..Default::default()
    };
    let mut env = facts.environment().ok_or("environment")?.value;
    env["canDock"] = false.into();
    page.set_environment(env)?;
    if let Some(state) = &job.state {
        page.receive_state(&json!({"v":2,"kind":"state.snapshot","rev":0,"state":state}))?;
    }
    Ok(page)
}

/// A page with motion needs a presentation surface; its first frame samples time zero and a
/// later frame samples `at`, so a state's `__time` (ms) pins an animation mid-flight.
fn layout(
    page: &mut lingxia_view::ViewSession,
    width: u16,
    height: u16,
    scale: f32,
    at: std::time::Duration,
) -> Result<lingxia_view::DisplayList, String> {
    match page.layout(width, height, scale) {
        Err(error) if error.contains("requires animation presentation scheduling") => {
            let zero = std::time::Duration::ZERO;
            let surface = lingxia_view::session::lifecycle::SurfaceTicket::new(1);
            page.set_animation_surface(Some(surface), zero)?;
            let first = page.prepare_view_animated(width, height, scale, true, None, zero, Ok)?;
            if at.is_zero() {
                return Ok(first);
            }
            // Time runs from the first frame's presentation.
            page.animation_presented(surface, page.generation(), zero)?;
            page.prepare_view_animated(width, height, scale, false, None, at, Ok)
        }
        other => other,
    }
}

fn run(job: &Job) -> Result<Value, String> {
    let source = std::fs::read_to_string(&job.module).map_err(|e| e.to_string())?;
    let (width, height) = (
        (f32::from(job.width) * job.scale) as u16,
        (f32::from(job.height) * job.scale) as u16,
    );
    let mut page = session(job, &source)?;
    let at = std::time::Duration::from_millis(
        job.state
            .as_ref()
            .and_then(|s| s["__time"].as_u64())
            .unwrap_or(0),
    );
    let mut display = layout(&mut page, width, height, job.scale, at)?;
    // `__picker: "<data-testid>"` opens the View's picker on that editor, as a tap would.
    if let Some(test_id) = job.state.as_ref().and_then(|s| s["__picker"].as_str()) {
        let node = display
            .nodes
            .iter()
            .find(|node| node.test_id == test_id)
            .ok_or_else(|| format!("__picker: no node {test_id}"))?;
        let (identity, select) = (node.identity.clone(), node.select.is_some());
        let opened = if select {
            page.open_select_now(&identity)?
        } else {
            page.open_picker_now(&identity, Some("2026-10-29"))?
        };
        if !opened {
            return Err(format!("__picker: {test_id} is no date or time editor"));
        }
        display = layout(&mut page, width, height, job.scale, at)?;
    }
    // Sources load from disk, as an adapter's loader would deliver them.
    if let Some(root) = &job.root {
        let mut loaded = false;
        for src in page.image_sources() {
            let bytes = std::fs::read(std::path::Path::new(root).join(&src))
                .map_err(|e| format!("{src}: {e}"))?;
            loaded |= page.load_image(&src, &bytes)?;
        }
        if loaded {
            display = layout(&mut page, width, height, job.scale, at)?;
        }
    }
    let cost = display.cost();
    let metrics = page.metrics().clone();
    if let Some(path) = &job.nodes {
        let json = serde_json::to_string_pretty(&display.nodes).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
    }
    let mut hash = Value::Null;
    if job.png.is_some() || job.hash {
        let frame = display.rasterize()?;
        if job.hash {
            hash = frame_hash(&frame.pixels).into();
        }
        if let Some(path) = &job.png {
            let png = frame.pixels.into_png().map_err(|e| e.to_string())?;
            std::fs::write(path, png).map_err(|e| e.to_string())?;
        }
    }
    // Timings are informative: the median of five cold frames.
    let mut times = Vec::new();
    for _ in 0..5 {
        let mut cold = session(job, &source)?;
        layout(&mut cold, width, height, job.scale, at)?;
        times.push(cold.metrics().total_us);
    }
    times.sort_unstable();
    Ok(json!({
        "cost": cost,
        "layoutPasses": metrics.layout_passes,
        "viewPasses": metrics.view_passes,
        "wireValues": metrics.wire_values,
        "medianUs": times[2],
        "hash": hash,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let jobs: Vec<Job> = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
    let results: Vec<Value> = jobs
        .iter()
        .map(|job| match run(job) {
            Ok(mut result) => {
                result["id"] = job.id.clone().into();
                result
            }
            Err(error) => json!({"id": job.id, "error": error}),
        })
        .collect();
    std::fs::write(&args[2], serde_json::to_string_pretty(&results)?)?;
    Ok(())
}
