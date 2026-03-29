use std::time::Duration;

pub type DataPoint = (Duration, f64);

pub fn min_max_downsample(samples: &[DataPoint], target_points: usize) -> Vec<DataPoint> {
    if samples.is_empty() {
        return Vec::new();
    }

    if target_points < 2 {
        return samples.last().copied().into_iter().collect();
    }

    if samples.len() <= target_points {
        return samples.to_vec();
    }

    let bucket_count = target_points / 2;
    if bucket_count == 0 {
        return vec![samples[0]];
    }

    let bucket_size = samples.len() as f64 / bucket_count as f64;
    let mut result = Vec::with_capacity(target_points);

    for b in 0..bucket_count {
        let start = (b as f64 * bucket_size) as usize;
        let end = (((b + 1) as f64) * bucket_size) as usize;
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

pub fn average_downsample(samples: &[DataPoint], target_points: usize) -> Vec<DataPoint> {
    if samples.is_empty() || target_points == 0 {
        return Vec::new();
    }

    if samples.len() <= target_points {
        return samples.to_vec();
    }

    let bucket_size = samples.len() as f64 / target_points as f64;
    let mut result = Vec::with_capacity(target_points);

    for b in 0..target_points {
        let start = (b as f64 * bucket_size) as usize;
        let end = (((b + 1) as f64) * bucket_size) as usize;
        let end = end.min(samples.len());

        if start >= end {
            continue;
        }

        let mut sum_t = 0.0f64;
        let mut sum_v = 0.0f64;
        let count = (end - start) as f64;

        for sample in &samples[start..end] {
            sum_t += sample.0.as_secs_f64();
            sum_v += sample.1;
        }

        result.push((Duration::from_secs_f64(sum_t / count), sum_v / count));
    }

    result
}
