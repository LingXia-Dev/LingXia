//! Times a compiled View page through Logic ticks on the host: each tick sends `{tick}` as a
//! state snapshot and lays the page out, as a device frame would. Driven by
//! packages/lingxia-view/corpus/bench.mjs.
//!   cargo run --release -p lingxia-view --example bench -- page.js [ticks] [state-json]
//! `state-json` is merged into every tick's state.

use serde_json::json;
use std::collections::BTreeMap;

fn percentile(values: &mut [u64], p: f64) -> f64 {
    values.sort_unstable();
    let index = ((values.len() as f64 - 1.0) * p).round() as usize;
    values[index] as f64 / 1000.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let source = std::fs::read_to_string(args.get(1).ok_or("usage: bench page.js [ticks]")?)?;
    let ticks: u64 = args.get(2).map_or(Ok(200), |t| t.parse())?;
    let extra: serde_json::Value = args
        .get(3)
        .map_or(Ok(json!({})), |s| serde_json::from_str(s))?;
    let (width, height, scale) = (393u16, 852u16, 2.0f32);
    // The dev clock lets the View runtime split its eval into phases.
    lingxia_view::engine::enable_dev_globals();
    let mut page = lingxia_view::ViewSession::new(&source)?;
    page.enable_eval_phases()?;
    let facts = lingxia_view::platform::DeviceFacts {
        width: (f32::from(width) * scale) as u32,
        height: (f32::from(height) * scale) as u32,
        scale,
        os: "bench".into(),
        smallest_width: f32::from(width),
        locale: "en".into(),
        animation_scale: 1.0,
        ..Default::default()
    };
    let mut env = facts.environment().ok_or("environment")?.value;
    env["canDock"] = false.into();
    page.set_environment(env)?;
    let (pw, ph) = (width * scale as u16, height * scale as u16);
    let names = [
        "total", "view", "eval", "decode", "transfer", "layout", "compose",
    ];
    let mut samples: Vec<Vec<u64>> = vec![Vec::new(); names.len()];
    let mut phases: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    let mut shape = (0, 0, 0);
    let mut gc_us = 0u64;
    // The first ticks warm the caches a device page also keeps between frames.
    let warm = 20;
    for tick in 0..ticks + warm {
        let mut state = extra.clone();
        state["tick"] = tick.into();
        page.receive_state(&json!({"v":2,"kind":"state.snapshot","rev":tick,"state":state}))?;
        page.layout(pw, ph, scale)?;
        // The host collects between frames, as a device worker does when idle.
        let started = std::time::Instant::now();
        page.collect_between_frames();
        if tick < warm {
            continue;
        }
        gc_us += started.elapsed().as_micros() as u64;
        let m = page.metrics();
        if let Some(p) = m.view_phases {
            // Vnode counts are scaled so they print as counts beside the ms columns.
            let spans = [
                ("render", p.render),
                ("resolve", p.resolve),
                ("recall", p.recall),
                ("rows", p.rows),
                ("encode", p.encode + p.windows),
                ("commit", p.commit),
                ("vnodes", p.h * 1000),
            ];
            for (name, value) in spans {
                phases.entry(name).or_default().push(value);
            }
        }
        shape = (m.view_passes, m.nodes, m.wire_values);
        let values = [
            m.total_us,
            m.view_us,
            m.view_eval_us,
            m.view_decode_us,
            m.view_transfer_us,
            m.layout_us,
            m.compose_us,
        ];
        for (sample, value) in samples.iter_mut().zip(values) {
            sample.push(value);
        }
    }
    let (passes, nodes, values) = shape;
    println!(
        "{ticks} ticks, last: {passes} View passes, {nodes} nodes, {values} wire values; ms p50/p95:"
    );
    for (name, sample) in names.iter().zip(&mut samples) {
        let p50 = percentile(sample, 0.5);
        let p95 = percentile(sample, 0.95);
        println!("  {name:<9} {p50:>7.2} {p95:>7.2}");
    }
    println!("eval phases (recall nests in resolve, rows in encode), ms p50/p95:");
    for (name, sample) in &mut phases {
        let (p50, p95) = (percentile(sample, 0.5), percentile(sample, 0.95));
        println!("  {name:<9} {p50:>7.2} {p95:>7.2}");
    }
    println!(
        "  gc          {:>7.2} mean per tick",
        gc_us as f64 / ticks as f64 / 1000.0
    );
    Ok(())
}
