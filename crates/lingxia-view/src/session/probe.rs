//! Debug-build bridge latencies in microseconds, logged per 100 samples (`lxdev logs --grep bridge-probe`).
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Probe {
    /// `setData` reached Rust → the View replica applied it.
    WriteApply,
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
    let elapsed = now_us().saturating_sub(since_us);
    let mut samples = SAMPLES.lock().unwrap_or_else(|p| p.into_inner());
    let entry = samples.entry((probe, gpu)).or_default();
    entry.push(elapsed);
    if entry.len() >= 100 {
        let mut sorted = std::mem::take(entry);
        sorted.sort_unstable();
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        log::info!(
            target: "lingxia_view",
            "bridge-probe {probe:?} {} n={} p50={}us p95={}us max={}us",
            if gpu { "gpu" } else { "webview" },
            sorted.len(),
            at(0.5),
            at(0.95),
            sorted[sorted.len() - 1]
        );
    }
}
