use readout_core::downsampling::DataPoint;

pub fn visible_point_budget(plot_width_points: f32) -> usize {
    ((plot_width_points.max(0.0) as usize) * 2).max(32)
}

pub fn downsample_visible_points(samples: &[DataPoint], target_points: usize) -> Vec<DataPoint> {
    if samples.is_empty() || target_points == 0 {
        return Vec::new();
    }

    if samples.len() <= target_points {
        return samples.to_vec();
    }

    if target_points == 1 {
        return vec![samples[samples.len() - 1]];
    }

    if target_points == 2 {
        let first = samples[0];
        let last = samples[samples.len() - 1];
        return if first == last {
            vec![first]
        } else {
            vec![first, last]
        };
    }

    let first = samples[0];
    let last = samples[samples.len() - 1];
    let middle = &samples[1..samples.len() - 1];
    let interior_budget = target_points - 2;

    if middle.len() <= interior_budget {
        return samples.to_vec();
    }

    let bucket_count = interior_budget.div_ceil(2);
    let bucket_size = middle.len() as f64 / bucket_count as f64;
    let mut result = Vec::with_capacity(target_points);
    result.push(first);

    for bucket_idx in 0..bucket_count {
        let start = (bucket_idx as f64 * bucket_size) as usize;
        let end = (((bucket_idx + 1) as f64) * bucket_size) as usize;
        let end = end.min(middle.len());

        if start >= end {
            continue;
        }

        let bucket = &middle[start..end];
        if bucket.len() == 1 {
            if result.len() < target_points - 1 {
                push_unique(&mut result, bucket[0]);
            }
            continue;
        }

        let mut min_idx = 0usize;
        let mut max_idx = 0usize;
        for idx in 1..bucket.len() {
            if bucket[idx].1.total_cmp(&bucket[min_idx].1).is_lt() {
                min_idx = idx;
            }
            if bucket[idx].1.total_cmp(&bucket[max_idx].1).is_gt() {
                max_idx = idx;
            }
        }

        let mut representatives = if min_idx == max_idx {
            vec![bucket[min_idx]]
        } else if bucket[min_idx].0 <= bucket[max_idx].0 {
            vec![bucket[min_idx], bucket[max_idx]]
        } else {
            vec![bucket[max_idx], bucket[min_idx]]
        };

        for representative in representatives.drain(..) {
            if result.len() >= target_points - 1 {
                break;
            }
            push_unique(&mut result, representative);
        }
    }

    push_unique(&mut result, last);
    result
}

fn push_unique(points: &mut Vec<DataPoint>, point: DataPoint) {
    if points.last().copied() != Some(point) {
        points.push(point);
    }
}

#[cfg(test)]
mod tests {
    use super::{downsample_visible_points, visible_point_budget};
    use readout_core::downsampling::DataPoint;
    use std::time::Duration;

    fn point(x: f64, y: f64) -> DataPoint {
        (Duration::from_secs_f64(x), y)
    }

    #[test]
    fn downsample_visible_points_preserves_first_and_last_point() {
        let samples = vec![
            point(0.0, 0.0),
            point(1.0, 4.0),
            point(2.0, 1.0),
            point(3.0, 5.0),
        ];
        let downsampled = downsample_visible_points(&samples, 2);

        assert_eq!(downsampled.first(), Some(&point(0.0, 0.0)));
        assert_eq!(downsampled.last(), Some(&point(3.0, 5.0)));
    }

    #[test]
    fn downsample_visible_points_keeps_monotonic_x_order() {
        let samples = vec![
            point(0.0, 2.0),
            point(1.0, 5.0),
            point(2.0, 1.0),
            point(3.0, 6.0),
        ];
        let downsampled = downsample_visible_points(&samples, 3);

        assert!(downsampled.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[test]
    fn downsample_visible_points_returns_raw_when_budget_is_large_enough() {
        let samples = vec![point(10.0, 1.0), point(11.0, 2.0), point(12.0, 3.0)];
        let downsampled = downsample_visible_points(&samples, 8);

        assert_eq!(downsampled, samples);
    }

    #[test]
    fn visible_point_budget_tracks_plot_width() {
        assert_eq!(visible_point_budget(0.0), 32);
        assert_eq!(visible_point_budget(640.0), 1280);
    }
}
