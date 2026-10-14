//! Per-step counters a system can read. Not a profiler and not a log.

use std::time::Duration;

use crate::RenderStats;

/// CPU time spent inside one [`crate::Game::step`], measured with [`std::time::Instant`].
///
/// Each field is how long that stage's systems, command flush, and event promotion took on the
/// CPU. [`StageDurations::fixed_update`] covers every fixed iteration in the step, including each
/// iteration's flush. It is not the fixed timestep, and it is not that timestep multiplied by the
/// iteration count.
///
/// These durations are unrelated to [`Diagnostics::frame`], which is the simulation delta passed
/// to `step`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageDurations {
    pub time: Duration,
    pub loads: Duration,
    pub fixed_update: Duration,
    pub update: Duration,
    pub animate: Duration,
    pub post_update: Duration,
    pub input_clear: Duration,
}

impl StageDurations {
    /// Sum of the seven stage measurements. CPU time inside `step`, not the frame delta.
    pub fn total(self) -> Duration {
        self.time + self.loads + self.fixed_update + self.update + self.animate + self.post_update + self.input_clear
    }
}

/// Frame counters overwritten in place. There is no history.
///
/// * `frame` is the simulation delta passed to [`crate::Game::step`]. It is not CPU time.
/// * `stages` is CPU time around each world stage. See [`StageDurations`].
/// * `fixed_steps` is how many fixed iterations ran. The fixed timestep itself stays on
///   [`engine_core::FixedTimestep`].
/// * `render` is the most recent successful render. `step` does not write it. During `step`, and
///   until the first successful render, it still describes the previous render, or zeros.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    /// Simulation `dt` for this step. Not the time the CPU spent in the step.
    pub frame: Duration,
    /// Fixed iterations that ran in this step (0 to 8).
    pub fixed_steps: u32,
    /// CPU time of each world stage. `fixed_update` is the whole fixed loop.
    pub stages: StageDurations,
    /// Entities in the world at the end of the step.
    pub entities: u32,
    /// Latest successful extract, frame, and GPU-cache counts.
    pub render: RenderStats,
    /// Command failures recorded during this step.
    pub command_failures: u32,
    /// Events dropped at the end of this step because they were still pending.
    pub discarded_events: u32,
}
