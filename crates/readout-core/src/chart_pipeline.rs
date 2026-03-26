use std::time::Duration;

/// A point in the chart: (timestamp, value).
pub type ChartPoint = (Duration, f64);

pub struct ChartPipeline {
    buffer: Vec<ChartPoint>,
    capacity: usize,
    write_pos: usize,
    count: usize,
    scratch: Vec<ChartPoint>,
}

impl ChartPipeline {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ChartPipeline capacity must be > 0");
        Self {
            buffer: vec![(Duration::ZERO, 0.0); capacity],
            capacity,
            write_pos: 0,
            count: 0,
            scratch: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, timestamp: Duration, value: f64) {
        self.buffer[self.write_pos] = (timestamp, value);
        self.write_pos = (self.write_pos + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.count = 0;
    }

    /// Query with automatic "now" = latest sample timestamp.
    pub fn query(&mut self, time_range: Duration, target_points: usize) -> Vec<ChartPoint> {
        if self.count == 0 {
            return Vec::new();
        }
        let latest_idx = if self.write_pos == 0 {
            self.capacity - 1
        } else {
            self.write_pos - 1
        };
        let now = self.buffer[latest_idx].0;
        self.query_with_now(time_range, target_points, now)
    }

    pub fn query_with_now(
        &mut self,
        time_range: Duration,
        target_points: usize,
        now: Duration,
    ) -> Vec<ChartPoint> {
        if self.count == 0 {
            return Vec::new();
        }

        let cutoff = now.saturating_sub(time_range);

        // Collect samples into scratch buffer (reused across calls)
        self.collect_samples_after(cutoff);

        if self.scratch.len() <= target_points {
            return self.scratch.clone();
        }

        // Min-max downsample
        Self::min_max_downsample(&self.scratch, target_points)
    }

    fn collect_samples_after(&mut self, cutoff: Duration) {
        self.scratch.clear();
        let start = if self.count < self.capacity {
            0
        } else {
            self.write_pos
        };

        for i in 0..self.count {
            let idx = (start + i) % self.capacity;
            let sample = self.buffer[idx];
            if sample.0 >= cutoff {
                self.scratch.push(sample);
            }
        }
    }

    fn min_max_downsample(samples: &[ChartPoint], target_points: usize) -> Vec<ChartPoint> {
        let bucket_count = target_points / 2;
        if bucket_count == 0 {
            // target_points < 2: return at least the last sample if available
            return samples.last().copied().into_iter().collect();
        }

        let bucket_size = samples.len() as f64 / bucket_count as f64;
        let mut result = Vec::with_capacity(target_points);

        for b in 0..bucket_count {
            let start = (b as f64 * bucket_size) as usize;
            let end = ((b + 1) as f64 * bucket_size) as usize;
            let end = end.min(samples.len());

            if start >= end {
                continue;
            }

            let mut min_idx = start;
            let mut max_idx = start;

            for i in start..end {
                if samples[i].1.total_cmp(&samples[min_idx].1).is_lt() {
                    min_idx = i;
                }
                if samples[i].1.total_cmp(&samples[max_idx].1).is_gt() {
                    max_idx = i;
                }
            }

            // Add in chronological order, deduplicate by index
            if min_idx == max_idx {
                result.push(samples[min_idx]);
            } else if samples[min_idx].0 <= samples[max_idx].0 {
                result.push(samples[min_idx]);
                result.push(samples[max_idx]);
            } else {
                result.push(samples[max_idx]);
                result.push(samples[min_idx]);
            }
        }

        result
    }
}
