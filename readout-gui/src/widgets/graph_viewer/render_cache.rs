use crate::widgets::graph_viewer::source_model::ViewerSourceId;
use egui_plot::PlotPoint;
use std::collections::HashMap;

/// Overscan margin as a fraction of the visible span (20% on each side).
const OVERSCAN_FRACTION: f64 = 0.2;

/// Maximum relative span difference for zoom compatibility (10%).
const ZOOM_TOLERANCE: f64 = 0.1;

/// Final plot-ready series for a compatible viewport.
struct ViewportSeriesCache {
    revision: u64,
    /// Overscan X range — visible range plus margin on each side.
    overscan_range: (f64, f64),
    cached_span: f64,
    point_budget: usize,
    series: Vec<egui_plot::PlotPoint>,
}

/// Top-level render cache owned by GraphViewerWindow.
pub struct RenderCache {
    caches: HashMap<ViewerSourceId, ViewportSeriesCache>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            caches: HashMap::new(),
        }
    }

    pub fn get_series(
        &self,
        source_id: ViewerSourceId,
        current_revision: u64,
        x_range: Option<(f64, f64)>,
        current_span: f64,
        point_budget: usize,
    ) -> Option<&[PlotPoint]> {
        let entry = self.caches.get(&source_id)?;

        if entry.revision != current_revision {
            return None;
        }
        if entry.point_budget != point_budget {
            return None;
        }
        if !spans_compatible(entry.cached_span, current_span) {
            return None;
        }
        if let Some((vis_min, vis_max)) = x_range
            && (vis_min < entry.overscan_range.0 || vis_max > entry.overscan_range.1)
        {
            return None;
        }

        Some(entry.series.as_slice())
    }

    pub fn store_series(
        &mut self,
        source_id: ViewerSourceId,
        revision: u64,
        x_range: Option<(f64, f64)>,
        span: f64,
        point_budget: usize,
        series: Vec<PlotPoint>,
    ) {
        let overscan = x_range
            .map(|(lo, hi)| overscan_range(lo, hi))
            .unwrap_or((f64::NEG_INFINITY, f64::INFINITY));

        self.caches.insert(
            source_id,
            ViewportSeriesCache {
                revision,
                overscan_range: overscan,
                cached_span: span,
                point_budget,
                series,
            },
        );
    }

    pub fn retain_sources(&mut self, active_ids: &[ViewerSourceId]) {
        self.caches.retain(|id, _| active_ids.contains(id));
    }

    pub fn invalidate_source(&mut self, source_id: ViewerSourceId) {
        self.caches.remove(&source_id);
    }
}

/// Compute the overscan range for a visible viewport.
fn overscan_range(visible_min: f64, visible_max: f64) -> (f64, f64) {
    let span = visible_max - visible_min;
    let margin = span * OVERSCAN_FRACTION;
    (visible_min - margin, visible_max + margin)
}

/// Check if two spans are compatible within zoom tolerance.
fn spans_compatible(cached_span: f64, current_span: f64) -> bool {
    if cached_span == 0.0 && current_span == 0.0 {
        return true; // both zero-span (single point) — compatible
    }
    if cached_span <= 0.0 || current_span <= 0.0 {
        return false;
    }
    let ratio = (current_span / cached_span - 1.0).abs();
    ratio <= ZOOM_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overscan_range_adds_margin_both_sides() {
        let (lo, hi) = overscan_range(100.0, 200.0);
        assert!((lo - 80.0).abs() < 1e-10);
        assert!((hi - 220.0).abs() < 1e-10);
    }

    #[test]
    fn spans_compatible_within_tolerance() {
        assert!(spans_compatible(100.0, 105.0));
        assert!(spans_compatible(100.0, 95.0));
    }

    #[test]
    fn spans_incompatible_beyond_tolerance() {
        assert!(!spans_compatible(100.0, 115.0));
        assert!(!spans_compatible(100.0, 85.0));
    }

    #[test]
    fn spans_incompatible_for_zero_or_negative() {
        assert!(!spans_compatible(0.0, 100.0));
        assert!(!spans_compatible(100.0, 0.0));
        assert!(!spans_compatible(-1.0, 100.0));
    }

    #[test]
    fn spans_compatible_both_zero() {
        assert!(spans_compatible(0.0, 0.0));
    }

    #[test]
    fn get_series_returns_none_on_empty_cache() {
        let cache = RenderCache::new();
        let result = cache.get_series(42, 1, Some((0.0, 100.0)), 100.0, 256);
        assert!(result.is_none());
    }

    #[test]
    fn get_series_returns_cached_after_store() {
        let mut cache = RenderCache::new();
        let series = vec![PlotPoint::new(1.0, 2.0), PlotPoint::new(3.0, 4.0)];
        cache.store_series(42, 1, Some((100.0, 200.0)), 100.0, 256, series.clone());

        let result = cache.get_series(42, 1, Some((100.0, 200.0)), 100.0, 256);
        assert_eq!(result, Some(series.as_slice()));
    }

    #[test]
    fn cache_invalidates_on_revision_change() {
        let mut cache = RenderCache::new();
        cache.store_series(
            1,
            5,
            Some((0.0, 100.0)),
            100.0,
            256,
            vec![PlotPoint::new(0.0, 1.0)],
        );

        assert!(
            cache
                .get_series(1, 5, Some((0.0, 100.0)), 100.0, 256)
                .is_some()
        );
        assert!(
            cache
                .get_series(1, 6, Some((0.0, 100.0)), 100.0, 256)
                .is_none()
        );
    }

    #[test]
    fn cache_invalidates_on_span_change() {
        let mut cache = RenderCache::new();
        cache.store_series(
            1,
            1,
            Some((0.0, 100.0)),
            100.0,
            256,
            vec![PlotPoint::new(0.0, 1.0)],
        );

        assert!(
            cache
                .get_series(1, 1, Some((0.0, 105.0)), 105.0, 256)
                .is_some()
        );
        assert!(
            cache
                .get_series(1, 1, Some((0.0, 150.0)), 150.0, 256)
                .is_none()
        );
    }

    #[test]
    fn cache_invalidates_on_budget_change() {
        let mut cache = RenderCache::new();
        cache.store_series(
            1,
            1,
            Some((0.0, 100.0)),
            100.0,
            256,
            vec![PlotPoint::new(0.0, 1.0)],
        );

        assert!(
            cache
                .get_series(1, 1, Some((0.0, 100.0)), 100.0, 256)
                .is_some()
        );
        assert!(
            cache
                .get_series(1, 1, Some((0.0, 100.0)), 100.0, 512)
                .is_none()
        );
    }

    #[test]
    fn cache_reuses_within_overscan() {
        let mut cache = RenderCache::new();
        cache.store_series(
            1,
            1,
            Some((100.0, 200.0)),
            100.0,
            256,
            vec![PlotPoint::new(100.0, 1.0)],
        );

        assert!(
            cache
                .get_series(1, 1, Some((90.0, 190.0)), 100.0, 256)
                .is_some()
        );
        assert!(
            cache
                .get_series(1, 1, Some((110.0, 210.0)), 100.0, 256)
                .is_some()
        );
        assert!(
            cache
                .get_series(1, 1, Some((70.0, 170.0)), 100.0, 256)
                .is_none()
        );
        assert!(
            cache
                .get_series(1, 1, Some((130.0, 230.0)), 100.0, 256)
                .is_none()
        );
    }

    #[test]
    fn repeated_get_with_unchanged_inputs_returns_same_cached_ref() {
        let mut cache = RenderCache::new();
        let series = vec![
            PlotPoint::new(1.0, 2.0),
            PlotPoint::new(3.0, 4.0),
            PlotPoint::new(5.0, 6.0),
        ];
        cache.store_series(1, 1, Some((0.0, 100.0)), 100.0, 256, series.clone());

        let first = cache
            .get_series(1, 1, Some((0.0, 100.0)), 100.0, 256)
            .unwrap();
        let second = cache
            .get_series(1, 1, Some((0.0, 100.0)), 100.0, 256)
            .unwrap();

        assert_eq!(first, series.as_slice());
        assert_eq!(first.as_ptr(), second.as_ptr());
    }

    #[test]
    fn retain_sources_drops_absent_ids() {
        let mut cache = RenderCache::new();
        cache.store_series(1, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 1.0)]);
        cache.store_series(2, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 2.0)]);
        cache.store_series(3, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 3.0)]);

        cache.retain_sources(&[1, 3]);

        assert!(cache.get_series(1, 1, None, 100.0, 256).is_some());
        assert!(cache.get_series(2, 1, None, 100.0, 256).is_none());
        assert!(cache.get_series(3, 1, None, 100.0, 256).is_some());
    }

    #[test]
    fn invalidate_source_removes_single_entry() {
        let mut cache = RenderCache::new();
        cache.store_series(1, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 1.0)]);
        cache.store_series(2, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 2.0)]);

        cache.invalidate_source(1);

        assert!(cache.get_series(1, 1, None, 100.0, 256).is_none());
        assert!(cache.get_series(2, 1, None, 100.0, 256).is_some());
    }
}
