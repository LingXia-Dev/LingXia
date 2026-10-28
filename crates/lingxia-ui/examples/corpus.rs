//! Renders compiled View pages headlessly for the corpus: deterministic cost counters per
//! page and viewport, informative timings, optional PNGs, and a visual fingerprint compared with
//! a committed one. Driven by packages/lingxia-ui/corpus/run.mjs.
//!   cargo run -p lingxia-ui --example corpus -- jobs.json results.json

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
    #[serde(default)]
    visual: Option<Visual>,
}

/// The frame's fingerprint against `reference`, a committed thumbnail of `CELL`-pixel block
/// means; `update` re-records it, a mismatch writes `diff`.
#[derive(Deserialize)]
struct Visual {
    reference: String,
    diff: String,
    #[serde(default)]
    update: bool,
}

/// Block size of the fingerprint, in device pixels, and the per-channel difference of a block
/// mean that counts as a change: anti-aliasing moves a mean by a few levels, a changed corner,
/// colour or line of text by more.
const CELL: usize = 16;
const TOLERANCE: u8 = 8;

fn thumbnail(pixels: &[u8], width: usize, height: usize) -> (usize, usize, Vec<u8>) {
    let (columns, rows) = (width.div_ceil(CELL), height.div_ceil(CELL));
    let mut out = vec![0u8; columns * rows * 4];
    for row in 0..rows {
        for column in 0..columns {
            let mut sum = [0u64; 4];
            let mut count = 0u64;
            for y in row * CELL..((row + 1) * CELL).min(height) {
                for x in column * CELL..((column + 1) * CELL).min(width) {
                    let at = (y * width + x) * 4;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += u64::from(pixels[at + channel]);
                    }
                    count += 1;
                }
            }
            let at = (row * columns + column) * 4;
            for channel in 0..4 {
                out[at + channel] = (sum[channel] / count.max(1)) as u8;
            }
        }
    }
    (columns, rows, out)
}

fn compare(frame: &vello_cpu::Pixmap, visual: &Visual) -> Result<Value, String> {
    let (width, height) = (usize::from(frame.width()), usize::from(frame.height()));
    let (columns, rows, cells) = thumbnail(frame.data_as_u8_slice(), width, height);
    let encode = |data: &[u8], width: usize, height: usize| {
        let mut pixmap = vello_cpu::Pixmap::new(width as u16, height as u16);
        pixmap.data_as_u8_slice_mut().copy_from_slice(data);
        pixmap.into_png().map_err(|e| e.to_string())
    };
    if visual.update {
        std::fs::write(&visual.reference, encode(&cells, columns, rows)?)
            .map_err(|e| e.to_string())?;
        return Ok(json!({"recorded": true}));
    }
    let Ok(file) = std::fs::read(&visual.reference) else {
        return Ok(json!({"missing": true}));
    };
    let reference =
        vello_cpu::Pixmap::from_png(std::io::Cursor::new(file)).map_err(|e| e.to_string())?;
    if usize::from(reference.width()) != columns || usize::from(reference.height()) != rows {
        return Ok(
            json!({"changed": columns * rows, "size": [reference.width(), reference.height()]}),
        );
    }
    let expected = reference.data_as_u8_slice();
    let changed: Vec<usize> = (0..columns * rows)
        .filter(|cell| {
            (0..4).any(|c| cells[cell * 4 + c].abs_diff(expected[cell * 4 + c]) > TOLERANCE)
        })
        .collect();
    if !changed.is_empty() {
        // The frame with every changed block outlined in red.
        let mut marked = frame.data_as_u8_slice().to_vec();
        for cell in &changed {
            let (x0, y0) = ((cell % columns) * CELL, (cell / columns) * CELL);
            for y in y0..(y0 + CELL).min(height) {
                for x in x0..(x0 + CELL).min(width) {
                    if y == y0
                        || x == x0
                        || y + 1 == (y0 + CELL).min(height)
                        || x + 1 == (x0 + CELL).min(width)
                    {
                        let at = (y * width + x) * 4;
                        marked[at..at + 4].copy_from_slice(&[255, 0, 0, 255]);
                    }
                }
            }
        }
        std::fs::write(&visual.diff, encode(&marked, width, height)?).map_err(|e| e.to_string())?;
    }
    Ok(json!({"changed": changed.len()}))
}

fn session(job: &Job, source: &str) -> Result<lingxia_ui::ViewSession, String> {
    let mut page = lingxia_ui::ViewSession::new(source)?;
    let facts = lingxia_ui::platform::DeviceFacts {
        width: (f32::from(job.width) * job.scale) as u32,
        height: (f32::from(job.height) * job.scale) as u32,
        scale: job.scale,
        os: "corpus".into(),
        dark: job.dark,
        smallest_width: f32::from(job.width.min(job.height)),
        locale: job.locale.clone().unwrap_or_else(|| "en".into()),
        animation_scale: 1.0,
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

/// A page with motion needs a presentation surface; its first frame samples time zero.
fn layout(
    page: &mut lingxia_ui::ViewSession,
    width: u16,
    height: u16,
    scale: f32,
) -> Result<lingxia_ui::DisplayList, String> {
    match page.layout(width, height, scale) {
        Err(error) if error.contains("requires animation presentation scheduling") => {
            let zero = std::time::Duration::ZERO;
            page.set_animation_surface(
                Some(lingxia_ui::session::lifecycle::SurfaceTicket::new(1)),
                zero,
            )?;
            page.prepare_view_animated(width, height, scale, true, None, zero, Ok)
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
    let display = layout(&mut page, width, height, job.scale)?;
    let cost = display.cost();
    let metrics = page.metrics().clone();
    if let Some(path) = &job.nodes {
        let json = serde_json::to_string_pretty(&display.nodes).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
    }
    let mut visual = Value::Null;
    if job.png.is_some() || job.visual.is_some() {
        let frame = display.rasterize()?;
        if let Some(job_visual) = &job.visual {
            visual = compare(&frame.pixels, job_visual)?;
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
        layout(&mut cold, width, height, job.scale)?;
        times.push(cold.metrics().total_us);
    }
    times.sort_unstable();
    Ok(json!({
        "cost": cost,
        "layoutPasses": metrics.layout_passes,
        "viewPasses": metrics.view_passes,
        "wireValues": metrics.wire_values,
        "medianUs": times[2],
        "visual": visual,
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
