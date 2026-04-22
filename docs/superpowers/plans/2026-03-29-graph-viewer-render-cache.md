# Graph Viewer Render Cache — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a viewport render cache to Graph Viewer so prepared plot series are reused across frames when source data, visible modes, viewport, and point budget haven't materially changed.

**Architecture:** Each visible source gets a viewport series cache storing final `Vec<egui_plot::PlotPoint>` data with overscan margins so cache hits can pass a borrowed slice directly into `egui_plot::Line::new(...)` without recloning plot series. Cache ownership lives in `GraphViewerWindow`, not `CsvDataStore`. Source revision tracking (`u64`) on each `ViewerSource` drives invalidation — no full-vector comparisons. The spec's two-layer model (filtered source cache + viewport cache) is staged: this plan implements the viewport cache only; filtered source cache is deferred to a follow-up.

**Tech Stack:** Rust, egui/egui_plot, `std::collections::HashMap`, `std::time::Duration`

**Spec:** `docs/superpowers/specs/2026-03-29-graph-viewer-render-cache-design.md`

**Deferred scope:**
- **Filtered Source Cache (Layer 1):** The spec describes a filtered source cache that stores pre-filtered `Vec<DataPoint>` keyed by revision. This plan defers Layer 1 to a follow-up — on viewport cache miss, the fallback path uses `query_points_in_view()` directly. The viewport cache alone eliminates per-frame rebuild for unchanged viewports.
- **Follow mode incremental reuse:** The spec describes append/drop optimization for live follow. Per the spec's "Safe Fallback" section, the initial implementation rebuilds the viewport series from the data store on cache miss. In follow mode, each new sample increments `render_revision`, causing a full rebuild — this is correct but not optimally incremental. A follow-up can add incremental tail reuse.

## Preflight

- If the execution worktree already contains uncommitted changes in files targeted by this plan, especially `readout-gui/src/widgets/graph_viewer/mod.rs`, do not mix them into the render-cache commits below.
- Before executing Task 1, either:
  - commit the existing changes as a separate prerequisite commit, or
  - move the plan execution to a clean worktree/branch.
- The per-task commits in this plan assume a clean baseline for the files they touch.
- Do not revert or discard pre-existing changes unless explicitly requested by the human partner.

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `readout-gui/src/widgets/graph_viewer/render_cache.rs` | Cache structs, reuse logic, overscan helpers |
| Modify | `readout-gui/src/widgets/graph_viewer/data_store.rs` | Add `render_revision: u64` to `ViewerSource`, increment on mutations |
| Modify | `readout-gui/src/widgets/graph_viewer/mod.rs` | Own `RenderCache`, integrate into render loop |

---

## Chunk 1: Source Revision Tracking

### Task 1: Add `render_revision` field to `ViewerSource`

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/data_store.rs:83-102` (ViewerSource struct)
- Modify: `readout-gui/src/widgets/graph_viewer/data_store.rs:1050+` (tests)

- [ ] **Step 1: Write the failing test — revision starts at 1 after initial load**

Add to the `#[cfg(test)] mod tests` block in `data_store.rs`:

```rust
#[test]
fn render_revision_is_one_after_initial_load() {
    let csv = write_temp_csv(&csv_with_value("2026-03-29T10:00:00Z", 1.0));
    let mut store = CsvDataStore::new();
    let id = store.load_csv_file(csv, false).unwrap();
    let source = store.source_by_id(id).unwrap();
    // Initial load: field starts at 0, refresh_source_metadata increments to 1
    assert_eq!(source.render_revision, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p readout-gui render_revision_is_one`
Expected: Compilation error — `render_revision` field does not exist.

- [ ] **Step 3: Add `render_revision` field to `ViewerSource` and all construction sites**

In `data_store.rs`, add field to the `ViewerSource` struct (after `is_live: bool`, line 101):

```rust
pub render_revision: u64,
```

Add `render_revision: 0,` in all four construction sites:
- `attach_runtime_device()` (line 156, after `is_live: false,`)
- `attach_live_csv()` (line 206, after `is_live: true,`)
- `attach_path_source()` (line 622, after `is_live,`)
- test helper `test_source()` (line 1139, after `is_live: false,`)

- [ ] **Step 4: Increment revision in `refresh_source_metadata()`**

At the end of `refresh_source_metadata()` (after `source.visible_modes = selected_modes;`, line 838):

```rust
source.render_revision += 1;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p readout-gui render_revision_is_one`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/data_store.rs
git commit -m "feat(gui): add render_revision field to ViewerSource"
```

---

### Task 2: Increment revision on remaining mutation paths

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/data_store.rs`

The `refresh_source_metadata()` path covers CSV load, live tail append, and file truncation/rotation. Two mutation paths bypass it: `push_runtime_sample()` (incremental metadata) and `set_mode_visible()`.

- [ ] **Step 1: Write test — revision increments on runtime sample push**

```rust
#[test]
fn render_revision_increments_on_runtime_sample() {
    let mut store = CsvDataStore::new();
    let device = DeviceId::Multimeter;
    let id = store.attach_runtime_device(device).unwrap();
    let rev_before = store.source_by_id(id).unwrap().render_revision;

    store.handle_runtime_event(&RuntimeEvent::Measurement {
        device,
        value: fake_measurement(device, 1.23),
    });

    let rev_after = store.source_by_id(id).unwrap().render_revision;
    assert!(rev_after > rev_before, "revision must increase after sample push");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p readout-gui render_revision_increments_on_runtime`
Expected: FAIL — `push_runtime_sample` does not increment revision.

- [ ] **Step 3: Increment revision in `push_runtime_sample()`**

In `push_runtime_sample()`, after `source.mode_filter_initialized = true;` (line 886), add:

```rust
source.render_revision += 1;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p readout-gui render_revision_increments_on_runtime`
Expected: PASS

- [ ] **Step 5: Write test — revision increments on mode visibility change**

```rust
#[test]
fn render_revision_increments_on_mode_visibility_change() {
    let csv = write_temp_csv(&csv_with_value("2026-03-29T10:00:00Z", 1.0));
    let mut store = CsvDataStore::new();
    let id = store.load_csv_file(csv, false).unwrap();
    let rev_before = store.source_by_id(id).unwrap().render_revision;

    store.set_mode_visible("DCV", false);

    let rev_after = store.source_by_id(id).unwrap().render_revision;
    assert!(rev_after > rev_before, "revision must increase after mode visibility change");
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cargo test -p readout-gui render_revision_increments_on_mode`
Expected: FAIL — `set_mode_visible` does not increment `render_revision`.

- [ ] **Step 7: Increment revision in `set_mode_visible()`**

In `set_mode_visible()` (line 345), add increment after the visibility toggle:

```rust
for source in &mut self.sources {
    if !source.modes.iter().any(|source_mode| source_mode == mode) {
        continue;
    }

    source.mode_filter_initialized = true;

    if visible {
        source.visible_modes.insert(mode.to_owned());
    } else {
        source.visible_modes.remove(mode);
    }
    source.render_revision += 1;
}
```

- [ ] **Step 8: Run all revision tests**

Run: `cargo test -p readout-gui render_revision`
Expected: All PASS

- [ ] **Step 9: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/data_store.rs
git commit -m "feat(gui): increment render_revision on runtime sample push and mode visibility"
```

---

## Chunk 2: Render Cache Module

### Task 3: Create `render_cache.rs` with cache types and helpers

**Files:**
- Create: `readout-gui/src/widgets/graph_viewer/render_cache.rs`
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs` (module declaration)

- [ ] **Step 1: Create module with types, helpers, and tests**

Create `render_cache.rs`:

```rust
use crate::widgets::graph_viewer::source_model::ViewerSourceId;
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

/// Compute the overscan range for a visible viewport.
fn overscan_range(visible_min: f64, visible_max: f64) -> (f64, f64) {
    let span = visible_max - visible_min;
    let margin = span * OVERSCAN_FRACTION;
    (visible_min - margin, visible_max + margin)
}

/// Check if two spans are compatible within zoom tolerance.
fn spans_compatible(cached_span: f64, current_span: f64) -> bool {
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
}
```

- [ ] **Step 2: Register the module in `mod.rs`**

In `readout-gui/src/widgets/graph_viewer/mod.rs`, add after `mod render_sampling;` (line 4):

```rust
mod render_cache;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p readout-gui render_cache`
Expected: All PASS (4 tests)

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/render_cache.rs readout-gui/src/widgets/graph_viewer/mod.rs
git commit -m "feat(gui): add render_cache module with cache types and overscan helpers"
```

---

### Task 4: Implement `RenderCache` public API

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/render_cache.rs`

- [ ] **Step 1: Write test — cache miss on empty cache**

```rust
#[test]
fn get_series_returns_none_on_empty_cache() {
    let cache = RenderCache::new();
    let result = cache.get_series(42, 1, Some((0.0, 100.0)), 100.0, 256);
    assert!(result.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p readout-gui get_series_returns_none`
Expected: Compilation error — `RenderCache::new()` and `get_series` don't exist.

- [ ] **Step 3: Implement `new()`, `get_series()`, `store_series()`**

```rust
use egui_plot::PlotPoint;

impl RenderCache {
    pub fn new() -> Self {
        Self {
            caches: HashMap::new(),
        }
    }

    /// Try to retrieve a cached plot series for the given source and viewport.
    /// Returns `None` on cache miss.
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
        // Viewport must be fully contained in cached overscan range
        if let Some((vis_min, vis_max)) = x_range {
            if vis_min < entry.overscan_range.0 || vis_max > entry.overscan_range.1 {
                return None;
            }
        }

        Some(entry.series.as_slice())
    }

    /// Store a freshly built plot series for a source and viewport.
    pub fn store_series(
        &mut self,
        source_id: ViewerSourceId,
        revision: u64,
        x_range: Option<(f64, f64)>,
        span: f64,
        point_budget: usize,
        series: Vec<PlotPoint>,
    ) {
        // When x_range is None (no viewport restriction), the cache covers everything.
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

    /// Drop caches for sources no longer present.
    pub fn retain_sources(&mut self, active_ids: &[ViewerSourceId]) {
        self.caches.retain(|id, _| active_ids.contains(id));
    }

    /// Force-invalidate a specific source's cache.
    pub fn invalidate_source(&mut self, source_id: ViewerSourceId) {
        self.caches.remove(&source_id);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p readout-gui get_series_returns_none`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/render_cache.rs
git commit -m "feat(gui): implement RenderCache get/store/invalidate API"
```

---

### Task 5: Cache invalidation and reuse tests

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/render_cache.rs`

- [ ] **Step 1: Write cache behavior tests**

Add to the test module:

```rust
#[test]
fn get_series_returns_cached_after_store() {
    use egui_plot::PlotPoint;

    let mut cache = RenderCache::new();
    let series = vec![PlotPoint::new(1.0, 2.0), PlotPoint::new(3.0, 4.0)];
    cache.store_series(42, 1, Some((100.0, 200.0)), 100.0, 256, series.clone());

    let result = cache.get_series(42, 1, Some((100.0, 200.0)), 100.0, 256);
    assert_eq!(result, Some(series.as_slice()));
}

#[test]
fn cache_invalidates_on_revision_change() {
    use egui_plot::PlotPoint;

    let mut cache = RenderCache::new();
    cache.store_series(1, 5, Some((0.0, 100.0)), 100.0, 256, vec![PlotPoint::new(0.0, 1.0)]);

    assert!(cache.get_series(1, 5, Some((0.0, 100.0)), 100.0, 256).is_some());
    assert!(cache.get_series(1, 6, Some((0.0, 100.0)), 100.0, 256).is_none());
}

#[test]
fn cache_invalidates_on_span_change() {
    use egui_plot::PlotPoint;

    let mut cache = RenderCache::new();
    cache.store_series(1, 1, Some((0.0, 100.0)), 100.0, 256, vec![PlotPoint::new(0.0, 1.0)]);

    // Small span change (5%) → hit
    assert!(cache.get_series(1, 1, Some((0.0, 105.0)), 105.0, 256).is_some());
    // Large span change (50%) → miss
    assert!(cache.get_series(1, 1, Some((0.0, 150.0)), 150.0, 256).is_none());
}

#[test]
fn cache_invalidates_on_budget_change() {
    use egui_plot::PlotPoint;

    let mut cache = RenderCache::new();
    cache.store_series(1, 1, Some((0.0, 100.0)), 100.0, 256, vec![PlotPoint::new(0.0, 1.0)]);

    assert!(cache.get_series(1, 1, Some((0.0, 100.0)), 100.0, 256).is_some());
    assert!(cache.get_series(1, 1, Some((0.0, 100.0)), 100.0, 512).is_none());
}

#[test]
fn cache_reuses_within_overscan() {
    use egui_plot::PlotPoint;

    let mut cache = RenderCache::new();
    // Visible: [100, 200], overscan: [80, 220]
    cache.store_series(1, 1, Some((100.0, 200.0)), 100.0, 256, vec![PlotPoint::new(100.0, 1.0)]);

    // Small pan inside overscan → hit
    assert!(cache.get_series(1, 1, Some((90.0, 190.0)), 100.0, 256).is_some());
    assert!(cache.get_series(1, 1, Some((110.0, 210.0)), 100.0, 256).is_some());

    // Pan outside overscan → miss
    assert!(cache.get_series(1, 1, Some((70.0, 170.0)), 100.0, 256).is_none());
    assert!(cache.get_series(1, 1, Some((130.0, 230.0)), 100.0, 256).is_none());
}

#[test]
fn repeated_get_with_unchanged_inputs_returns_same_cached_ref() {
    use egui_plot::PlotPoint;

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
    use egui_plot::PlotPoint;

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
    use egui_plot::PlotPoint;

    let mut cache = RenderCache::new();
    cache.store_series(1, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 1.0)]);
    cache.store_series(2, 1, None, 100.0, 256, vec![PlotPoint::new(0.0, 2.0)]);

    cache.invalidate_source(1);

    assert!(cache.get_series(1, 1, None, 100.0, 256).is_none());
    assert!(cache.get_series(2, 1, None, 100.0, 256).is_some());
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p readout-gui render_cache`
Expected: All PASS (12 tests total)

- [ ] **Step 3: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/render_cache.rs
git commit -m "test(gui): add cache invalidation, reuse, and cleanup tests"
```

---

## Chunk 3: Integration Into Render Loop

### Task 6: Add `RenderCache` to `GraphViewerWindow`

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs:60-115` (struct + constructor)

- [ ] **Step 1: Add field and import**

In `mod.rs`, add import (after `mod render_cache;`):

```rust
use self::render_cache::RenderCache;
```

Add field to `GraphViewerWindow` struct (after `last_error: Option<String>,`):

```rust
render_cache: RenderCache,
```

Initialize in `new()` (after `last_error: None,`):

```rust
render_cache: RenderCache::new(),
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p readout-gui`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/mod.rs
git commit -m "feat(gui): add RenderCache field to GraphViewerWindow"
```

---

### Task 7: Wire cache into `render_chart()`

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs:399-421` (per-source render loop)

- [ ] **Step 1: Replace per-source rendering with cache-aware path**

The existing per-source block inside the `egui_plot::Plot::show()` closure (lines 399-421) currently calls `self.data_store.query_points_in_view()` on every frame. Replace it with a cache-first approach.

The closure already captures `&mut self` (it writes to `self.overlay.cursor_pos` and `self.hovered_cursor`). Rust 2021 disjoint field captures allow `&self.data_store` (immutable) and `&mut self.render_cache` (mutable) simultaneously within the closure since they are distinct fields. The `self.cursor_info_for_point()` method call borrows `&self` temporarily and its borrow ends before the per-source loop.

Replace lines 399-421 with:

```rust
let current_span = plot_query.x_range
    .map(|(lo, hi)| (hi - lo).abs())
    .unwrap_or(0.0);

for file in self.data_store.files() {
    if !file.visible {
        continue;
    }

    // Try cache hit first
    if let Some(cached) = self.render_cache.get_series(
        file.id,
        file.render_revision,
        plot_query.x_range,
        current_span,
        plot_query.target_points,
    ) {
        if !cached.is_empty() {
            plot_ui.line(
                egui_plot::Line::new(file.label.clone(), cached)
                    .stroke(egui::Stroke::new(1.5, file.color)),
            );
        }
        continue;
    }

    // Cache miss — rebuild from data store
    let source_id = file.id;
    let revision = file.render_revision;
    let label = file.label.clone();
    let color = file.color;

    let points = self.data_store.query_points_in_view(
        source_id,
        plot_query.x_range,
        plot_query.target_points,
    );

    let series: Vec<egui_plot::PlotPoint> = points
        .into_iter()
        .map(|(time, value)| egui_plot::PlotPoint::new(time.as_secs_f64(), value))
        .collect();

    self.render_cache.store_series(
        source_id,
        revision,
        plot_query.x_range,
        current_span,
        plot_query.target_points,
        series,
    );

    if let Some(cached) = self.render_cache.get_series(
        source_id,
        revision,
        plot_query.x_range,
        current_span,
        plot_query.target_points,
    ) {
        if !cached.is_empty() {
            plot_ui.line(
                egui_plot::Line::new(label, cached)
                    .stroke(egui::Stroke::new(1.5, color)),
            );
        }
    }
}
```

**Borrow checker note:** If compilation fails due to the closure needing simultaneous access to `self.data_store` (via `files()` iterator) and `self.render_cache`, split the borrows before the closure:

```rust
let render_cache = &mut self.render_cache;
let data_store = &self.data_store;
```

Then use `data_store` and `render_cache` instead of `self.data_store` and `self.render_cache` inside the closure. This requires also extracting `self.overlay` and `self.hovered_cursor` access outside the closure or via explicit split borrows. Only apply this pattern if the simpler approach above doesn't compile.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p readout-gui`
Expected: Compiles. Apply borrow-splitting fallback if needed.

- [ ] **Step 3: Run existing tests**

Run: `cargo test -p readout-gui`
Expected: All existing tests PASS — the cache is transparent to existing behavior.

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/mod.rs
git commit -m "feat(gui): wire render cache into chart rendering loop"
```

---

### Task 8: Cache invalidation on source removal and visibility toggle

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs:230-248` (action handlers)

- [ ] **Step 1: Add cache invalidation to action handlers**

In the `ViewerAction::RemoveSource` handler (line 241), add cache invalidation:

```rust
ViewerAction::RemoveSource(source_id) => {
    self.data_store.remove_source(source_id);
    self.render_cache.invalidate_source(source_id);
    if self.data_store.file_count() == 0 {
        self.overlay = overlay::OverlayState::default();
        self.following = false;
    }
    self.hovered_cursor = None;
}
```

In the `ViewerAction::ToggleSourceVisibility` handler (line 230), add cache invalidation. A source becoming visible again could serve stale data if revision and viewport happen to match:

```rust
ViewerAction::ToggleSourceVisibility(source_id) => {
    if let Some(file) = self
        .data_store
        .files_mut()
        .iter_mut()
        .find(|file| file.id == source_id)
    {
        file.visible = !file.visible;
    }
    self.render_cache.invalidate_source(source_id);
    self.hovered_cursor = None;
}
```

- [ ] **Step 2: Add stale cache cleanup in render path**

In `render_chart()`, before the `egui_plot::Plot::new()` call (before line 378), add:

```rust
let active_ids: Vec<_> = self.data_store.files().iter().map(|f| f.id).collect();
self.render_cache.retain_sources(&active_ids);
```

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo test -p readout-gui`
Expected: All PASS.

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/mod.rs
git commit -m "feat(gui): invalidate render cache on source removal and visibility toggle"
```

---

## Chunk 4: Verification

### Task 9: Full test suite, clippy, and build

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p readout-gui`
Expected: All tests PASS, including all existing viewport-aware rendering tests.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p readout-gui -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run workspace build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Final commit if any cleanup needed**

Only if previous steps revealed issues requiring fixes.
