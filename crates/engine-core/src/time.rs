use std::time::Duration;

/// Frame timing, stored as a resource and refreshed once per frame by the platform loop.
#[derive(Clone, Copy, Debug, Default)]
pub struct Time {
    delta: Duration,
    elapsed: Duration,
    frame: u64,
}

impl Time {
    /// Advances the clock by `delta`. Called by the game loop; tests can call it directly.
    pub fn advance(&mut self, delta: Duration) {
        self.delta = delta;
        self.elapsed += delta;
        self.frame += 1;
    }
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Replaces the delta systems observe. Does not change [`Time::elapsed_secs`] or [`Time::frame`].
    ///
    /// `Game::step` uses this so each fixed-update iteration sees the fixed step, then restores
    /// the frame delta.
    pub fn set_delta(&mut self, delta: Duration) {
        self.delta = delta;
    }
    pub fn delta_secs(&self) -> f32 {
        self.delta.as_secs_f32()
    }
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
}

/// Accumulates variable frame time and hands out fixed-size steps, so simulation can be
/// deterministic regardless of frame rate.
#[derive(Clone, Copy, Debug)]
pub struct FixedTimestep {
    step: Duration,
    accumulator: Duration,
    max_steps_per_frame: u32,
}

impl FixedTimestep {
    pub fn new(hz: u32) -> Self {
        Self { step: Duration::from_secs_f64(1.0 / hz as f64), accumulator: Duration::ZERO, max_steps_per_frame: 8 }
    }
    pub fn step(&self) -> Duration {
        self.step
    }
    pub fn step_secs(&self) -> f32 {
        self.step.as_secs_f32()
    }

    /// Adds a frame's elapsed time and returns how many fixed steps to run. Capped so a long
    /// stall cannot trigger a spiral of death; excess time is dropped.
    pub fn accumulate(&mut self, delta: Duration) -> u32 {
        self.accumulator += delta;
        let mut n = 0;
        while self.accumulator >= self.step && n < self.max_steps_per_frame {
            self.accumulator -= self.step;
            n += 1;
        }
        if n == self.max_steps_per_frame {
            self.accumulator = Duration::ZERO;
        }
        n
    }

    /// Fraction of the next step already accumulated, in `0.0..=1.0`, for render interpolation.
    ///
    /// A zero step, a non-finite ratio, or the leftover dropped by the eight-step stall cap yields
    /// `0.0`. Values that rounding would push past 1 stay at 1.
    pub fn alpha(&self) -> f32 {
        let step = self.step.as_secs_f32();
        if step <= 0.0 || !step.is_finite() {
            return 0.0;
        }
        let alpha = self.accumulator.as_secs_f32() / step;
        if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_steps_are_independent_of_frame_rate() {
        let mut a = FixedTimestep::new(60);
        let mut b = FixedTimestep::new(60);
        let slow: u32 = (0..30).map(|_| a.accumulate(Duration::from_millis(100))).sum();
        let fast: u32 = (0..300).map(|_| b.accumulate(Duration::from_millis(10))).sum();
        assert!((slow as i32 - fast as i32).abs() <= 1, "{slow} vs {fast}");
        assert!((slow as i32 - 180).abs() <= 1);
    }

    #[test]
    fn stalls_are_capped() {
        let mut t = FixedTimestep::new(60);
        assert_eq!(t.accumulate(Duration::from_secs(10)), 8);
        assert_eq!(t.alpha(), 0.0, "the dropped leftover does not leave a partial step");
        assert_eq!(t.accumulate(Duration::ZERO), 0);
        assert_eq!(t.alpha(), 0.0);
    }

    #[test]
    fn alpha_tracks_the_leftover_and_stays_finite() {
        let mut t = FixedTimestep::new(60);
        assert_eq!(t.alpha(), 0.0, "the first frame has no accumulated time");

        let step = t.step();
        assert_eq!(t.accumulate(Duration::ZERO), 0);
        assert_eq!(t.alpha(), 0.0);

        assert_eq!(t.accumulate(step / 2), 0);
        assert!((t.alpha() - 0.5).abs() < 1e-4, "one partial step and no fixed update: {}", t.alpha());

        let rest = t.step - t.accumulator;
        assert_eq!(t.accumulate(rest), 1, "the partial step completes");
        assert_eq!(t.alpha(), 0.0);

        assert_eq!(t.accumulate(step * 2 + step / 2), 2);
        assert!((t.alpha() - 0.5).abs() < 1e-5, "two consumed steps leave the same fraction: {}", t.alpha());

        let over =
            FixedTimestep { step: Duration::from_secs(1), accumulator: Duration::from_secs(3), max_steps_per_frame: 8 };
        assert_eq!(over.alpha(), 1.0);
        let zero_step =
            FixedTimestep { step: Duration::ZERO, accumulator: Duration::from_millis(1), max_steps_per_frame: 8 };
        assert_eq!(zero_step.alpha(), 0.0);
        let empty = FixedTimestep { step: Duration::ZERO, accumulator: Duration::ZERO, max_steps_per_frame: 8 };
        assert_eq!(empty.alpha(), 0.0);

        let mut rolling = FixedTimestep::new(60);
        for n in 0..32 {
            let steps = rolling.accumulate(Duration::from_millis(n * 3));
            let alpha = rolling.alpha();
            assert!(alpha.is_finite() && (0.0..=1.0).contains(&alpha), "{alpha} after {steps} steps");
        }
    }
}
