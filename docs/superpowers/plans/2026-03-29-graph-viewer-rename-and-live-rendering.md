# Graph Viewer Rename And Live Rendering Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the `csv_viewer` feature to `Graph Viewer` at the feature boundary and replace whole-history downsampling with viewport-aware rendering so live tails stay fast without drawing false line segments.

**Architecture:** Keep the existing viewer store, toolbar, overlays, and CSV/runtime source model, but move the feature into a `graph_viewer` module boundary. Add a viewer-local rendering helper that filters to the visible X range, computes a width-based point budget, and downsamples only inside that window while preserving monotonic X ordering and first/last visible points.

**Tech Stack:** Rust, egui/eframe, egui_plot, egui_phosphor, chrono, readout_core `DataPoint`

**Spec:** `docs/superpowers/specs/2026-03-29-graph-viewer-rename-and-live-rendering-design.md`

---

## File Structure

### New Files

- `readout-gui/src/widgets/graph_viewer/mod.rs` — renamed viewer module entrypoint and feature-level constants
- `readout-gui/src/widgets/graph_viewer/data_store.rs` — moved data store with visible-range query support
- `readout-gui/src/widgets/graph_viewer/info_bar.rs` — moved info bar
- `readout-gui/src/widgets/graph_viewer/overlay.rs` — moved overlay tools
- `readout-gui/src/widgets/graph_viewer/source_model.rs` — moved source model
- `readout-gui/src/widgets/graph_viewer/viewer_toolbar.rs` — moved toolbar UI
- `readout-gui/src/widgets/graph_viewer/render_sampling.rs` — viewport-aware range filtering and safe downsampling helpers

### Modified Files

- `readout-gui/src/widgets/mod.rs` — export `graph_viewer` instead of `csv_viewer`
- `readout-gui/src/widgets/toolbar.rs` — rename `OpenCsvViewer`, switch tooltip text, use a distinct Graph Viewer icon
- `readout-gui/src/app.rs` — rename app field/action wiring to `GraphViewerWindow`

### Files Intentionally Left Alone

- `crates/readout-core/src/downsampling.rs` — do not change shared dashboard downsampling in this plan; keep the viewer-specific sampling logic local to Graph Viewer
- CSV-specific function names such as `attach_live_csv`, CSV import/export helpers, and `CsvRecord` types — these remain CSV-named because they still describe CSV-backed sources
- `readout-gui/src/widgets/csv_viewer/CLAUDE.md` — this file is currently untracked; do not delete, overwrite, or move it during the feature rename

### Boundary Decisions

- Rename the feature boundary, not the CSV source APIs
- Implement viewport-aware rendering inside the viewer module, not in shared `readout-core`
- Prefer a new helper file for render sampling over making `data_store.rs` even larger

---

## Chunk 1: Feature Rename Boundary

### Task 1: Rename the feature-level module and identifiers to Graph Viewer

**Files:**
- Create: `readout-gui/src/widgets/graph_viewer/mod.rs`
- Create: `readout-gui/src/widgets/graph_viewer/data_store.rs`
- Create: `readout-gui/src/widgets/graph_viewer/info_bar.rs`
- Create: `readout-gui/src/widgets/graph_viewer/overlay.rs`
- Create: `readout-gui/src/widgets/graph_viewer/source_model.rs`
- Create: `readout-gui/src/widgets/graph_viewer/viewer_toolbar.rs`
- Modify: `readout-gui/src/widgets/mod.rs`
- Modify: `readout-gui/src/widgets/toolbar.rs`
- Modify: `readout-gui/src/app.rs`
- Test: `readout-gui/src/widgets/toolbar.rs`
- Test: `readout-gui/src/widgets/graph_viewer/mod.rs`

- [ ] **Step 1: Write failing rename tests**

Add narrow tests that lock down the new feature identity.

In `readout-gui/src/widgets/toolbar.rs`, add:

```rust
#[test]
fn toolbar_exposes_open_graph_viewer_action() {
    let action = ToolbarAction::OpenGraphViewer;
    assert!(matches!(action, ToolbarAction::OpenGraphViewer));
}

#[test]
fn graph_viewer_toolbar_button_uses_graph_viewer_copy_and_new_icon() {
    assert_eq!(GRAPH_VIEWER_TOOLTIP, "Graph Viewer (Cmd+L)");
    assert_eq!(GRAPH_VIEWER_ICON, icons::PRESENTATION_CHART);
    assert_ne!(GRAPH_VIEWER_ICON, icons::CHART_LINE);
}
```

In the moved viewer module test section, add:

```rust
#[test]
fn graph_viewer_constants_use_renamed_feature_ids() {
    assert_eq!(GRAPH_VIEWER_WINDOW_TITLE, "Graph Viewer");
    assert_eq!(GRAPH_VIEWER_VIEWPORT_ID, "graph_viewer");
    assert_eq!(GRAPH_VIEWER_PLOT_ID, "graph_viewer_plot");
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui toolbar_exposes_open_graph_viewer_action -- --exact`

Run: `cargo test -p readout-gui graph_viewer_toolbar_button_uses_graph_viewer_copy_and_new_icon -- --exact`

Run: `cargo test -p readout-gui graph_viewer_constants_use_renamed_feature_ids -- --exact`

Expected: FAIL to compile because `OpenGraphViewer`, `GRAPH_VIEWER_ICON`, `GRAPH_VIEWER_TOOLTIP`, and the Graph Viewer constants do not exist yet.

- [ ] **Step 3: Perform the feature-boundary rename**

Rename tracked Rust files from `readout-gui/src/widgets/csv_viewer/` to `readout-gui/src/widgets/graph_viewer/`.

Important safety rule:

- move only tracked Rust source files
- leave the untracked `readout-gui/src/widgets/csv_viewer/CLAUDE.md` in place
- do not try to delete the old directory if the untracked file keeps it alive

In `readout-gui/src/widgets/mod.rs`, change:

```rust
pub mod csv_viewer;
```

to:

```rust
pub mod graph_viewer;
```

In `readout-gui/src/widgets/toolbar.rs`, rename the action and introduce testable constants:

```rust
pub const GRAPH_VIEWER_ICON: &str = icons::PRESENTATION_CHART;
pub const GRAPH_VIEWER_TOOLTIP: &str = "Graph Viewer (Cmd+L)";

pub enum ToolbarAction {
    // ...
    OpenGraphViewer,
}
```

Use those constants in `show_title_bar()`.

In `readout-gui/src/app.rs`, rename the field and action wiring:

```rust
graph_viewer: widgets::graph_viewer::GraphViewerWindow,
```

and:

```rust
widgets::toolbar::ToolbarAction::OpenGraphViewer => {
    self.graph_viewer.open = true;
}
```

In `readout-gui/src/widgets/graph_viewer/mod.rs`, rename the type and add constants:

```rust
pub const GRAPH_VIEWER_WINDOW_TITLE: &str = "Graph Viewer";
pub const GRAPH_VIEWER_VIEWPORT_ID: &str = "graph_viewer";
pub const GRAPH_VIEWER_PLOT_ID: &str = "graph_viewer_plot";

pub struct GraphViewerWindow {
    // previous CsvViewerWindow fields
}
```

Use those constants in:

```rust
.with_title(GRAPH_VIEWER_WINDOW_TITLE)
egui::ViewportId::from_hash_of(GRAPH_VIEWER_VIEWPORT_ID)
egui_plot::Plot::new(GRAPH_VIEWER_PLOT_ID)
```

Do not rename CSV-specific functions such as `attach_live_csv()` or labels such as `Tail CSV`.

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui toolbar_exposes_open_graph_viewer_action -- --exact`

Run: `cargo test -p readout-gui graph_viewer_toolbar_button_uses_graph_viewer_copy_and_new_icon -- --exact`

Run: `cargo test -p readout-gui graph_viewer_constants_use_renamed_feature_ids -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/mod.rs readout-gui/src/widgets/toolbar.rs readout-gui/src/app.rs readout-gui/src/widgets/graph_viewer
git commit -m "refactor(gui): rename csv viewer feature to graph viewer"
```

---

## Chunk 2: Viewport-Aware Sampling Foundation

### Task 2: Add viewer-local sampling helpers that preserve visible shape

**Files:**
- Create: `readout-gui/src/widgets/graph_viewer/render_sampling.rs`
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs`
- Test: `readout-gui/src/widgets/graph_viewer/render_sampling.rs`

- [ ] **Step 1: Write failing sampling tests**

Create `readout-gui/src/widgets/graph_viewer/render_sampling.rs` with tests first:

```rust
use readout_core::downsampling::DataPoint;
use std::time::Duration;

fn point(x: f64, y: f64) -> DataPoint {
    (Duration::from_secs_f64(x), y)
}

#[test]
fn downsample_visible_points_preserves_first_and_last_point() {
    let samples = vec![point(0.0, 0.0), point(1.0, 4.0), point(2.0, 1.0), point(3.0, 5.0)];
    let downsampled = downsample_visible_points(&samples, 2);

    assert_eq!(downsampled.first(), Some(&point(0.0, 0.0)));
    assert_eq!(downsampled.last(), Some(&point(3.0, 5.0)));
}

#[test]
fn downsample_visible_points_keeps_monotonic_x_order() {
    let samples = vec![point(0.0, 2.0), point(1.0, 5.0), point(2.0, 1.0), point(3.0, 6.0)];
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
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui downsample_visible_points_preserves_first_and_last_point -- --exact`

Run: `cargo test -p readout-gui downsample_visible_points_keeps_monotonic_x_order -- --exact`

Run: `cargo test -p readout-gui downsample_visible_points_returns_raw_when_budget_is_large_enough -- --exact`

Run: `cargo test -p readout-gui visible_point_budget_tracks_plot_width -- --exact`

Expected: FAIL to compile because `render_sampling.rs` and its helpers do not exist yet.

- [ ] **Step 3: Implement the local sampling helper**

In `readout-gui/src/widgets/graph_viewer/render_sampling.rs`, implement:

```rust
pub fn visible_point_budget(plot_width_points: f32) -> usize {
    ((plot_width_points.max(0.0) as usize) * 2).max(32)
}

pub fn downsample_visible_points(samples: &[DataPoint], target_points: usize) -> Vec<DataPoint> {
    // preserve raw points when already small enough
    // preserve first and last visible point
    // choose representative bucket points in monotonic x order
}
```

Implementation rules:

- if `samples.len() <= target_points`, return raw samples unchanged
- always include `samples[0]` and `samples[last]`
- downsample only the middle points
- bucket middle points by count
- if a bucket contributes multiple representatives, sort them by `x` before pushing
- deduplicate identical neighboring points if bucketing produces duplicates

Keep this helper Graph Viewer-specific. Do not modify `crates/readout-core/src/downsampling.rs`.

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui downsample_visible_points_preserves_first_and_last_point -- --exact`

Run: `cargo test -p readout-gui downsample_visible_points_keeps_monotonic_x_order -- --exact`

Run: `cargo test -p readout-gui downsample_visible_points_returns_raw_when_budget_is_large_enough -- --exact`

Run: `cargo test -p readout-gui visible_point_budget_tracks_plot_width -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/render_sampling.rs readout-gui/src/widgets/graph_viewer/mod.rs
git commit -m "feat(graph-viewer): add viewport-aware sampling helpers"
```

### Task 3: Teach the data store to query points for the visible X range

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/data_store.rs`
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs`
- Test: `readout-gui/src/widgets/graph_viewer/data_store.rs`

- [ ] **Step 1: Write failing visible-range query tests**

In `readout-gui/src/widgets/graph_viewer/data_store.rs`, add tests that describe the new contract:

```rust
#[test]
fn query_points_in_view_returns_only_visible_samples() {
    let path = write_temp_csv(concat!(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
        "2026-03-29T10:00:00Z,Multimeter,1.0,V,DCV,false,false,false\n",
        "2026-03-29T10:00:01Z,Multimeter,2.0,V,DCV,false,false,false\n",
        "2026-03-29T10:00:02Z,Multimeter,3.0,V,DCV,false,false,false\n",
    ));

    let mut store = CsvDataStore::new();
    let source_id = store.load_csv_file(path.clone(), false).unwrap();
    let points = store.query_points_in_view(source_id, Some((1.0, 2.0)), 64);

    assert_eq!(points.len(), 2);
    assert_eq!(points[0].1, 2.0);
    assert_eq!(points[1].1, 3.0);
}

#[test]
fn query_points_in_view_keeps_newest_local_live_shape() {
    let path = write_temp_csv(&build_dense_live_csv());

    let mut store = CsvDataStore::new();
    let source_id = store.attach_live_csv(DeviceId::Multimeter, path.clone()).unwrap();
    let points = store.query_points_in_view(source_id, Some((95.0, 100.0)), 256);

    let xs: Vec<f64> = points.iter().map(|(x, _)| x.as_secs_f64()).collect();
    assert!(xs.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(xs.last().copied().unwrap() >= 100.0);
}
```

Add a helper `build_dense_live_csv()` that creates long history plus a small newest oscillating segment so the test can catch the false-diagonal bug.

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui query_points_in_view_returns_only_visible_samples -- --exact`

Run: `cargo test -p readout-gui query_points_in_view_keeps_newest_local_live_shape -- --exact`

Expected: FAIL because `query_points_in_view()` does not exist yet.

- [ ] **Step 3: Implement visible-range querying in the store**

In `readout-gui/src/widgets/graph_viewer/data_store.rs`, add:

```rust
pub fn query_points_in_view(
    &self,
    source_id: ViewerSourceId,
    x_range: Option<(f64, f64)>,
    target_points: usize,
) -> Vec<DataPoint>
```

Implementation rules:

- look up the source by stable source ID
- filter to visible source modes first
- when `x_range` is `Some((x_min, x_max))`, include only points with `sample.x` inside the inclusive visible range
- if `x_range` is `None`, fall back to all visible source points
- convert to `DataPoint`
- if the filtered slice is small enough, return raw points
- otherwise call `render_sampling::downsample_visible_points()`

Keep the existing `query_points()` until the render path is switched, but make it a thin compatibility wrapper or remove it once all call sites are updated in the same commit.

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui query_points_in_view_returns_only_visible_samples -- --exact`

Run: `cargo test -p readout-gui query_points_in_view_keeps_newest_local_live_shape -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/data_store.rs readout-gui/src/widgets/graph_viewer/mod.rs readout-gui/src/widgets/graph_viewer/render_sampling.rs
git commit -m "feat(graph-viewer): query and downsample only visible plot ranges"
```

---

## Chunk 3: Graph Viewer Render Integration

### Task 4: Drive Graph Viewer plotting from viewport bounds and plot width

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs`
- Test: `readout-gui/src/widgets/graph_viewer/mod.rs`

- [ ] **Step 1: Write failing integration tests around target-point calculation and fallback**

In `readout-gui/src/widgets/graph_viewer/mod.rs`, add small unit tests around any new helper functions you extract from rendering, for example:

```rust
#[test]
fn plot_query_uses_visible_bounds_when_available() {
    let bounds = Some((10.0, 20.0));
    let width = Some(500.0);

    let query = build_plot_query(bounds, width);

    assert_eq!(query.x_range, Some((10.0, 20.0)));
    assert_eq!(query.target_points, 1000);
}

#[test]
fn plot_query_falls_back_to_safe_budget_when_width_is_missing() {
    let query = build_plot_query(Some((0.0, 5.0)), None);
    assert_eq!(query.target_points, 256);
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui plot_query_uses_visible_bounds_when_available -- --exact`

Run: `cargo test -p readout-gui plot_query_falls_back_to_safe_budget_when_width_is_missing -- --exact`

Expected: FAIL because the extracted query helper does not exist yet.

- [ ] **Step 3: Integrate viewport-aware querying into the plot loop**

In `readout-gui/src/widgets/graph_viewer/mod.rs`:

- add a small internal helper such as:

```rust
struct PlotQuery {
    x_range: Option<(f64, f64)>,
    target_points: usize,
}
```

- derive `x_range` from:

```rust
let bounds = plot_ui.plot_bounds();
let x_range = bounds.range_x();
```

- derive the width-based budget from:

```rust
let plot_width = plot_ui.response().rect.width();
```

- build the query before iterating files
- replace:

```rust
let points = self.data_store.query_points(file.id, 2_000);
```

with:

```rust
let points = self.data_store.query_points_in_view(file.id, query.x_range, query.target_points);
```

Fallback rules:

- when width is unavailable or non-positive, use a safe fixed budget such as `256`
- do not fall back to whole-history downsampling just because the plot is temporarily missing width info

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui plot_query_uses_visible_bounds_when_available -- --exact`

Run: `cargo test -p readout-gui plot_query_falls_back_to_safe_budget_when_width_is_missing -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/mod.rs readout-gui/src/widgets/graph_viewer/data_store.rs
git commit -m "fix(graph-viewer): render from visible plot window"
```

### Task 5: Add a regression test for the broken Tail CSV shape

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/data_store.rs`
- Modify: `readout-gui/src/widgets/graph_viewer/mod.rs`
- Test: `readout-gui/src/widgets/graph_viewer/data_store.rs`

- [ ] **Step 1: Write the failing regression test**

Add a test that explicitly models the screenshot failure:

```rust
#[test]
fn live_tail_visible_window_does_not_skip_intermediate_new_points() {
    let path = write_temp_csv(&build_dense_live_csv());

    let mut store = CsvDataStore::new();
    let source_id = store.attach_live_csv(DeviceId::Multimeter, path.clone()).unwrap();

    let points = store.query_points_in_view(source_id, Some((96.0, 100.5)), 32);
    let ys: Vec<f64> = points.iter().map(|(_, y)| *y).collect();

    assert!(ys.len() >= 5);
    assert!(ys.windows(2).any(|pair| pair[0] < pair[1]));
    assert!(ys.windows(2).any(|pair| pair[0] > pair[1]));
}
```

This test should use a helper dataset whose newest visible segment oscillates enough that skipping intermediate points would collapse it into one or two false diagonals.

- [ ] **Step 2: Run the targeted regression test to verify it fails**

Run: `cargo test -p readout-gui live_tail_visible_window_does_not_skip_intermediate_new_points -- --exact`

Expected: FAIL if the visible-range query still drops too much newest detail.

- [ ] **Step 3: Make the minimal fix needed for the regression**

If the new viewport-aware integration from Task 4 still over-simplifies the newest tail, tune only the Graph Viewer-local logic:

- raise the raw-point threshold for small visible windows
- or ensure the visible budget is at least the visible sample count when the window is already small

Do not change shared dashboard chart behavior.

- [ ] **Step 4: Run the targeted regression test to verify it passes**

Run: `cargo test -p readout-gui live_tail_visible_window_does_not_skip_intermediate_new_points -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/graph_viewer/data_store.rs readout-gui/src/widgets/graph_viewer/mod.rs readout-gui/src/widgets/graph_viewer/render_sampling.rs
git commit -m "test(graph-viewer): lock live tail shape regression"
```

### Task 6: Full verification and cleanup

**Files:**
- Modify: `readout-gui/src/widgets/graph_viewer/*` as needed from prior tasks
- Modify: `readout-gui/src/widgets/toolbar.rs`
- Modify: `readout-gui/src/app.rs`

- [ ] **Step 1: Run the full Graph Viewer test suite**

Run: `cargo test -p readout-gui`

Expected: PASS

- [ ] **Step 2: Run lint verification**

Run: `cargo clippy -p readout-gui -- -D warnings`

Expected: PASS

- [ ] **Step 3: Do a manual smoke check**

Verify locally in the app:

- toolbar opens `Graph Viewer` and no longer says `CSV Viewer`
- open action uses `icons::PRESENTATION_CHART`, not `icons::CHART_LINE`
- `Tail CSV` still exists as a CSV-specific action
- with a live tail attached, panning/zooming a narrow newest window does not collapse the final oscillation into a false diagonal

- [ ] **Step 4: Commit final polish**

```bash
git add readout-gui/src/widgets/mod.rs readout-gui/src/widgets/toolbar.rs readout-gui/src/app.rs readout-gui/src/widgets/graph_viewer
git commit -m "fix(graph-viewer): keep live tails accurate under downsampling"
```
