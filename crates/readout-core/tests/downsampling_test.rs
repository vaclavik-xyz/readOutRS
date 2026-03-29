use readout_core::downsampling::{DataPoint, average_downsample, min_max_downsample};
use std::time::Duration;

fn point(seconds: u64, value: f64) -> DataPoint {
    (Duration::from_secs(seconds), value)
}

#[test]
fn min_max_preserves_peaks() {
    let samples = vec![
        point(0, 1.0),
        point(1, 2.0),
        point(2, 3.0),
        point(3, 100.0),
        point(4, 4.0),
        point(5, 5.0),
        point(6, 6.0),
        point(7, 7.0),
    ];

    let downsampled = min_max_downsample(&samples, 4);

    assert!(downsampled.iter().any(|(_, value)| *value == 100.0));
}

#[test]
fn min_max_returns_empty_for_empty_input() {
    let downsampled = min_max_downsample(&[], 10);

    assert!(downsampled.is_empty());
}

#[test]
fn min_max_returns_all_when_fewer_than_target() {
    let samples = vec![point(0, 1.0), point(1, 2.0), point(2, 3.0)];

    let downsampled = min_max_downsample(&samples, 10);

    assert_eq!(downsampled, samples);
}

#[test]
fn min_max_returns_latest_sample_for_target_points_zero() {
    let samples = vec![point(0, 1.0), point(1, 2.0), point(2, 3.0)];

    let downsampled = min_max_downsample(&samples, 0);

    assert_eq!(downsampled, vec![point(2, 3.0)]);
}

#[test]
fn min_max_returns_latest_sample_for_target_points_one() {
    let samples = vec![point(0, 1.0), point(1, 2.0), point(2, 3.0)];

    let downsampled = min_max_downsample(&samples, 1);

    assert_eq!(downsampled, vec![point(2, 3.0)]);
}

#[test]
fn average_downsample_reduces_count() {
    let samples = vec![
        point(0, 1.0),
        point(1, 3.0),
        point(2, 5.0),
        point(3, 7.0),
        point(4, 9.0),
        point(5, 11.0),
    ];

    let downsampled = average_downsample(&samples, 3);

    assert!(downsampled.len() < samples.len());
    assert_eq!(downsampled.len(), 3);
}
