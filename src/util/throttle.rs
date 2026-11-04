use std::time::{Duration, Instant};

/// Answers "has `interval` passed since I last said yes?".
#[derive(Debug, Clone, Copy)]
pub struct Throttle {
    interval: Duration,
    last: Instant,
}

impl Throttle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: Instant::now(),
        }
    }

    pub fn ready(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last) < self.interval {
            return false;
        }
        self.last = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_only_after_the_interval() {
        let mut throttle = Throttle::new(Duration::from_millis(20));
        assert!(!throttle.ready());
        std::thread::sleep(Duration::from_millis(25));
        assert!(throttle.ready());
        assert!(!throttle.ready());
    }
}
