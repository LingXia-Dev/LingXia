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
    /// Visible presentations that showed animated values in the last second.
    pub animation_frames: usize,
    /// Removed nodes still drawn while their exit transition runs.
    #[serde(skip_serializing_if = "is_zero")]
    pub ghosts: usize,
    /// Nodes moving to a new layout position under `transition-layout`.
    #[serde(skip_serializing_if = "is_zero")]
    pub layout_motions: usize,
    /// Deterministic counters of the frame, compared against the page limits.
    pub cost: crate::FrameCost,
    /// Where View eval went, when the page opted its dev clock into phases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_phases: Option<ViewPhases>,
}

/// Eval time by runtime phase in µs; spans nest (recall in resolve, rows in encode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewPhases {
    pub render: u64,
    pub resolve: u64,
    pub recall: u64,
    pub rows: u64,
    pub encode: u64,
    pub windows: u64,
    pub commit: u64,
    /// Vnodes created.
    pub h: u64,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
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

/// Share of a limit past which a page is warned, once per limit.
const BUDGET_WARN: f64 = 0.75;

impl crate::ViewSession {
    /// Warns once per page per limit when a frame uses at least three quarters of it.
    pub(crate) fn check_budgets(&mut self) {
        use crate::protocol::limits;
        let metrics = &self.metrics;
        let cost = &metrics.cost;
        let fails = "past it the frame fails";
        let uses: [(&'static str, u64, u64, &str); 7] = [
            ("nodes", metrics.nodes as u64, limits::NODES as u64, fails),
            (
                "wireValues",
                metrics.wire_values as u64,
                limits::WIRE_VALUES as u64,
                fails,
            ),
            (
                "vectorWork",
                cost.vector_elements as u64,
                limits::VECTOR_WORK as u64,
                fails,
            ),
            (
                "visibleImageBytes",
                cost.image_pixels * 4,
                limits::VISIBLE_IMAGE_BYTES as u64,
                "past it further images show their placeholder",
            ),
            (
                "ghosts",
                metrics.ghosts as u64,
                limits::GHOSTS as u64,
                "past it further removals vanish at once",
            ),
            (
                "layoutMotions",
                metrics.layout_motions as u64,
                limits::LAYOUT_MOTIONS as u64,
                "past it further moves land at once",
            ),
            (
                "outlineGlyphs",
                cost.outline_glyphs as u64,
                limits::OUTLINE_GLYPHS as u64,
                "each is filled as a path, so frames take longer",
            ),
        ];
        for (name, used, limit, consequence) in uses {
            if used as f64 >= BUDGET_WARN * limit as f64 && self.budget_warned.insert(name) {
                let share = (used as f64 / limit as f64 * 100.0).round();
                self.budget_warnings.push(format!(
                    "UI_BUDGET: this page uses {share}% of `{name}` ({used} of {limit}); {consequence}"
                ));
            }
        }
    }

    /// Budget warnings since the last call, for the host to log.
    pub fn take_budget_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.budget_warnings)
    }
}
