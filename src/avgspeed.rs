use std::collections::VecDeque;
use std::ops::*;
use std::time::{Duration, Instant};

/// moving (rolling) average
pub struct RollingAverage<T> {
    hist: VecDeque<T>,
    sum: T,
    size: usize,
}

impl<T> RollingAverage<T>
// moments like this I miss dating duck typing
where
    T: AddAssign
        + SubAssign
        + Div
        + std::convert::From<u64>
        + std::convert::From<<T as std::ops::Div>::Output>
        + Copy,
{
    pub fn new(size: usize) -> Self {
        RollingAverage {
            hist: VecDeque::with_capacity(size),
            sum: 0_u64.into(),
            size,
        }
    }
    pub fn add(&mut self, val: T) {
        self.hist.push_back(val);
        self.sum += val;
        if self.hist.len() > self.size {
            self.sum -= self.hist.pop_front().unwrap();
        }
    }
    pub fn get(&self) -> T {
        (self.sum / (self.hist.len() as u64).into()).into()
    }
}

pub struct AvgSpeed {
    avg: RollingAverage<u64>,
    prev_bytes: u64,
    last_chunk: Instant,
}

impl AvgSpeed {
    pub fn new() -> Self {
        AvgSpeed {
            avg: RollingAverage::new(100),
            prev_bytes: 0,
            last_chunk: Instant::now(),
        }
    }
    pub fn add(&mut self, total_bytes: u64) {
        let db = total_bytes - self.prev_bytes;
        self.avg.add(get_speed(
            db,
            &Instant::now().duration_since(self.last_chunk),
        ));
        self.last_chunk = Instant::now();
        self.prev_bytes = total_bytes;
    }
    pub fn get(&self) -> u64 {
        self.avg.get()
    }
}

pub fn get_speed(x: u64, ela: &Duration) -> u64 {
    if *ela >= Duration::from_nanos(1) && x < std::u64::MAX / 1_000_000_000 {
        x * 1_000_000_000 / ela.as_nanos() as u64
    } else if *ela >= Duration::from_micros(1) && x < std::u64::MAX / 1_000_000 {
        x * 1_000_000 / ela.as_micros() as u64
    } else if *ela >= Duration::from_millis(1) && x < std::u64::MAX / 1_000 {
        x * 1_000 / ela.as_millis() as u64
    } else if *ela >= Duration::from_secs(1) {
        x / ela.as_secs()
    } else {
        // what the hell are you?
        std::u64::MAX
    }
}
