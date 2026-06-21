use readout_core::chart_pipeline::*;
use std::time::Duration;

#[test]
fn empty_pipeline_returns_empty() {
    let mut pipeline = ChartPipeline::new(1000);
    let points = pipeline.query(Duration::from_secs(120), 800);
    assert!(points.is_empty());
}

#[test]
fn push_and_query_returns_points() {
    let mut pipeline = ChartPipeline::new(1000);
    let base = Duration::from_secs(100);
    for i in 0..10 {
        pipeline.push(base + Duration::from_millis(i * 100), i as f64);
    }
    let points = pipeline.query(Duration::from_secs(120), 800);
    assert_eq!(points.len(), 10);
}

#[test]
fn ring_buffer_wraps_at_capacity() {
    let mut pipeline = ChartPipeline::new(5);
    let base = Duration::from_secs(100);
    for i in 0..10 {
        pipeline.push(base + Duration::from_millis(i * 100), i as f64);
    }
    // Only last 5 should remain
    let points = pipeline.query(Duration::from_secs(120), 100);
    assert_eq!(points.len(), 5);
    assert!((points[0].1 - 5.0).abs() < 0.001);
}

#[test]
fn time_filter_excludes_old_samples() {
    let mut pipeline = ChartPipeline::new(1000);
    let now = Duration::from_secs(200);
    // Add samples: some old, some recent
    for i in 0..100 {
        let ts = Duration::from_secs(100 + i);
        pipeline.push(ts, i as f64);
    }
    // Query last 30 seconds from ts=200
    let points = pipeline.query_with_now(Duration::from_secs(30), 800, now);
    // Should only have samples from ts=170..199
    assert_eq!(points.len(), 30);
    assert_eq!(points.first(), Some(&(Duration::from_secs(170), 70.0)));
    assert_eq!(points.last(), Some(&(Duration::from_secs(199), 99.0)));
}

#[test]
fn downsampling_reduces_to_target() {
    let mut pipeline = ChartPipeline::new(10000);
    let base = Duration::from_secs(0);
    for i in 0..5000 {
        pipeline.push(base + Duration::from_millis(i * 20), (i as f64).sin());
    }
    let points = pipeline.query(Duration::from_secs(120), 200);
    // Should be around 200 points (min-max pairs)
    assert!(points.len() <= 400); // min-max doubles the count
    assert!(points.len() >= 100);
}

#[test]
fn downsampling_preserves_peaks() {
    let mut pipeline = ChartPipeline::new(1000);
    let base = Duration::from_secs(0);
    // Create data with a clear spike at index 50
    for i in 0..100 {
        let value = if i == 50 { 100.0 } else { 1.0 };
        pipeline.push(base + Duration::from_millis(i * 100), value);
    }
    let points = pipeline.query(Duration::from_secs(30), 20);
    // The spike should be preserved
    let max_val = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    assert!((max_val - 100.0).abs() < 0.001);
}

#[test]
fn time_filter_works_after_ring_buffer_wrap() {
    let mut pipeline = ChartPipeline::new(5);

    for i in 0..10 {
        pipeline.push(Duration::from_secs(i), i as f64);
    }

    let points = pipeline.query_with_now(Duration::from_secs(2), 10, Duration::from_secs(9));
    let values: Vec<f64> = points.into_iter().map(|(_, v)| v).collect();

    assert_eq!(values, vec![7.0, 8.0, 9.0]);
}

#[test]
fn query_with_now_target_points_zero_returns_latest_visible_sample() {
    let mut pipeline = ChartPipeline::new(10);
    pipeline.push(Duration::from_secs(10), 1.0);
    pipeline.push(Duration::from_secs(20), 2.0);
    pipeline.push(Duration::from_secs(30), 3.0);

    let points = pipeline.query_with_now(Duration::from_secs(60), 0, Duration::from_secs(30));

    assert_eq!(points, vec![(Duration::from_secs(30), 3.0)]);
}

#[test]
fn query_with_now_target_points_one_returns_latest_visible_sample() {
    let mut pipeline = ChartPipeline::new(10);
    pipeline.push(Duration::from_secs(10), 1.0);
    pipeline.push(Duration::from_secs(20), 2.0);
    pipeline.push(Duration::from_secs(30), 3.0);

    let points = pipeline.query_with_now(Duration::from_secs(60), 1, Duration::from_secs(30));

    assert_eq!(points, vec![(Duration::from_secs(30), 3.0)]);
}
