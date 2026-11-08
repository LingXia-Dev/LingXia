//! Debug-build bridge and input latencies in microseconds, logged per batch of samples
//! (`lxdev logs --grep bridge-probe` / `--grep input-probe`).
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Probe {
    /// `setData` reached Rust → the View replica applied it.
    WriteApply,
    /// The platform's input timestamp → the host received the event.
    InputDispatch,
    /// Input received → the frame that shows it was requested from the worker.
    InputSlot,
    /// Frame requested → the worker started it.
    InputQueue,
    /// The worker building the frame: input, View step, layout and paint.
    InputBuild,
    /// Built → the vsync that hands it to the display.
    InputHold,
    /// Vsync → the adapter reported the frame handed over.
    InputPresent,
    /// Handed over → the refresh it shows on.
    InputScanout,
    /// Input received → the refresh it shows on.
    InputToPresent,
    /// The render thread's work for a frame that shows input: scene build, encode, submit
    /// and present.
    DrawScene,
    DrawEncode,
    DrawSubmit,
    DrawPresent,
}

impl Probe {
    fn is_input(self) -> bool {
        !matches!(self, Probe::WriteApply)
    }
}

/// Wall-clock microseconds: comparable across crates and threads; 0 in release builds.
pub(crate) fn now_us() -> u64 {
    if !cfg!(debug_assertions) {
        return 0;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64)
}

/// Elapsed microseconds per probe and transport (`true` = GPU).
type Samples = HashMap<(Probe, bool), Vec<u64>>;

static SAMPLES: LazyLock<Mutex<Samples>> = LazyLock::new(Default::default);

/// Record `since_us` → now for `probe` on the GPU (`gpu`) or WebView transport.
pub(crate) fn record(probe: Probe, gpu: bool, since_us: u64) {
    if !cfg!(debug_assertions) || since_us == 0 {
        return;
    }
    sample(probe, gpu, now_us().saturating_sub(since_us));
}

/// Record one already measured span; input spans log per 50 so one drag gives a line.
pub(crate) fn sample(probe: Probe, gpu: bool, elapsed: u64) {
    if !cfg!(debug_assertions) {
        return;
    }
    let mut samples = SAMPLES.lock().unwrap_or_else(|p| p.into_inner());
    let entry = samples.entry((probe, gpu)).or_default();
    entry.push(elapsed);
    if entry.len() >= if probe.is_input() { 50 } else { 100 } {
        let mut sorted = std::mem::take(entry);
        sorted.sort_unstable();
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        log::info!(
            target: "lingxia_view",
            "{} {probe:?} {} n={} p50={}us p95={}us max={}us",
            if probe.is_input() { "input-probe" } else { "bridge-probe" },
            if gpu { "gpu" } else { "webview" },
            sorted.len(),
            at(0.5),
            at(0.95),
            sorted[sorted.len() - 1]
        );
    }
}
