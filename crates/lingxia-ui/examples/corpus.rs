//! Renders compiled View pages headlessly for the corpus: deterministic cost counters per
//! page and viewport, informative timings, optional PNGs. Driven by packages/lingxia-ui/corpus/run.mjs.
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

/// A page with gated motion needs a presentation surface; its first frame samples time zero.
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
    if let Some(path) = &job.png {
        let frame = display.rasterize()?;
        let png = frame.pixels.into_png().map_err(|e| e.to_string())?;
        std::fs::write(path, png).map_err(|e| e.to_string())?;
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
