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

    /// Fraction of the next step already accumulated, for render interpolation.
    pub fn alpha(&self) -> f32 {
        self.accumulator.as_secs_f32() / self.step.as_secs_f32()
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
        assert_eq!(t.accumulate(Duration::ZERO), 0);
    }
}
