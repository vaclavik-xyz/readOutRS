use std::time::Duration;

/// A point in the chart: (timestamp, value).
pub type ChartPoint = (Duration, f64);

pub struct ChartPipeline {
    buffer: Vec<ChartPoint>,
    capacity: usize,
    write_pos: usize,
    count: usize,
}

impl ChartPipeline {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![(Duration::ZERO, 0.0); capacity],
            capacity,
            write_pos: 0,
            count: 0,
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
    pub fn query(&self, time_range: Duration, target_points: usize) -> Vec<ChartPoint> {
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
        &self,
        time_range: Duration,
        target_points: usize,
        now: Duration,
    ) -> Vec<ChartPoint> {
        if self.count == 0 {
            return Vec::new();
        }

        let cutoff = now.saturating_sub(time_range);

        // Collect samples in chronological order within time range
        let samples = self.ordered_samples_after(cutoff);

        if samples.len() <= target_points {
            return samples;
        }

        // Min-max downsample
        Self::min_max_downsample(&samples, target_points)
    }

    fn ordered_samples_after(&self, cutoff: Duration) -> Vec<ChartPoint> {
        let mut result = Vec::new();
        let start = if self.count < self.capacity {
            0
        } else {
            self.write_pos
        };

        for i in 0..self.count {
            let idx = (start + i) % self.capacity;
            let sample = self.buffer[idx];
            if sample.0 >= cutoff {
                result.push(sample);
            }
        }
        result
    }

    fn min_max_downsample(samples: &[ChartPoint], target_points: usize) -> Vec<ChartPoint> {
        let bucket_count = target_points / 2;
        if bucket_count == 0 {
            return Vec::new();
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

            let mut min_sample = samples[start];
            let mut max_sample = samples[start];

            for &sample in &samples[start..end] {
                if sample.1 < min_sample.1 {
                    min_sample = sample;
                }
                if sample.1 > max_sample.1 {
                    max_sample = sample;
                }
            }

            // Add in chronological order
            if min_sample.0 <= max_sample.0 {
                result.push(min_sample);
                if min_sample != max_sample {
                    result.push(max_sample);
                }
            } else {
                result.push(max_sample);
                if min_sample != max_sample {
                    result.push(min_sample);
                }
            }
        }

        result
    }
}
