use std::thread;
use std::time;

pub struct Limiter {
    delay: time::Duration,
    catch_up: bool,
    target: time::Instant,
}

impl Limiter {
    pub fn new(freq: f64, catch_up: bool) -> Self {
        assert!(freq > 0.0, "Frequency of limiters must be greater than 0.");

        Self {
            delay: time::Duration::from_secs_f64(1.0 / freq),
            catch_up,
            target: time::Instant::now(),
        }
    }

    pub fn wait_if_early(&mut self) {
        let current = time::Instant::now();

        if current < self.target {
            thread::sleep(self.target - current);
        }

        self.target = if self.catch_up {
            self.target.checked_add(self.delay).unwrap_or({
                eprintln!("Failed to catch-up limiter.");
                time::Instant::now()
            })
        } else {
            time::Instant::now()
        }
    }

    pub fn reset(&mut self) {
        self.target = time::Instant::now();
    }
}
