use std::time::{Duration, Instant};

pub struct Timer {
    start: Instant,
    last_frame: Instant,
    delta_time: Duration,
    elapsed: Duration,
    frame_count: u64,
}

impl Timer {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last_frame: now,
            delta_time: Duration::ZERO,
            elapsed: Duration::ZERO,
            frame_count: 0,
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        self.delta_time = now - self.last_frame;
        self.elapsed = now - self.start;
        self.last_frame = now;
        self.frame_count += 1;
    }

    pub fn delta_time(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }

    pub fn elapsed(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn fps(&self) -> f32 {
        if self.elapsed.as_secs_f32() > 0.0 {
            self.frame_count as f32 / self.elapsed.as_secs_f32()
        } else {
            0.0
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}
