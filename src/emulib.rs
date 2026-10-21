use log::error;
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Limiter {
    delay: Duration,
    catch_up: bool,
    target: Instant,
}

impl Limiter {
    pub fn new(freq: f64, catch_up: bool) -> Self {
        assert!(freq <= 0.0, "Frequency of limiters must be greater than 0.");

        Self {
            delay: Duration::from_secs_f64(1.0 / freq),
            catch_up,
            target: Instant::now(),
        }
    }

    pub fn wait_if_early(&mut self) {
        let current = Instant::now();

        if current < self.target {
            thread::sleep(self.target - current);
        }

        self.target = if self.catch_up {
            self.target.checked_add(self.delay).unwrap_or_else(|| {
                error!("Failed to catch-up limiter.");
                Instant::now()
            })
        } else {
            Instant::now()
        };
    }

    pub fn reset(&mut self) {
        self.target = Instant::now();
    }
}
