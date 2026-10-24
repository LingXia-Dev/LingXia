/// CPU wall times for the last successful frame, in microseconds.
/// `total_us` encloses all work; `text_us` is a subset of `layout_us`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameMetrics {
    pub generation: u64,
    pub total_us: u64,
    pub view_us: u64,
    pub view_eval_us: u64,
    pub view_transfer_us: u64,
    pub view_decode_us: u64,
    pub wire_values: usize,
    pub layout_us: u64,
    pub text_us: u64,
    pub compose_us: u64,
    pub build_us: u64,
    pub view_passes: u32,
    pub layout_passes: u32,
    pub nodes: usize,
    pub paints: usize,
}

pub(crate) fn micros(start: std::time::Instant) -> u64 {
    start.elapsed().as_micros().min(u64::MAX as u128) as u64
}

#[derive(Default)]
pub(crate) struct ViewTimings {
    pub eval_us: u64,
    pub transfer_us: u64,
    pub decode_us: u64,
    pub values: usize,
}
impl FrameMetrics {
    pub(crate) fn add_view(&mut self, timings: ViewTimings) {
        self.view_eval_us += timings.eval_us;
        self.view_transfer_us += timings.transfer_us;
        self.view_decode_us += timings.decode_us;
        self.wire_values += timings.values;
    }
}
