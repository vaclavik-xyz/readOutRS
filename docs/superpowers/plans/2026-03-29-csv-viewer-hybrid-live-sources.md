# CSV Viewer Hybrid Live Sources Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend CSV Viewer so it can overlay historical CSV files, explicit live CSV tails, and direct runtime live sources while fixing current follow/hover/tooltip robustness issues.

**Architecture:** Keep the existing viewport-based viewer and overlay tools, but replace the file-only store with a source registry that normalizes data from three source kinds into one plotting model. Runtime live history stays viewer-local, starts on explicit attach, and shares the same rendering/export path as CSV sources. Live CSV tailing remains file-based, but becomes explicit and faster.

**Tech Stack:** Rust, egui/eframe, egui_plot, chrono, tokio runtime events, rfd

**Spec:** `docs/superpowers/specs/2026-03-29-csv-viewer-hybrid-live-sources-design.md`

---

## File Structure

### New Files

- `readout-gui/src/widgets/csv_viewer/source_model.rs` — shared viewer source types (`ViewerSourceId`, `ViewerSourceKind`, `XDomain`, `ViewerSample`, `SourceStatus`)

### Modified Files

- `readout-gui/src/widgets/csv_viewer/mod.rs` — viewer actions, follow behavior, runtime event entrypoint, axis formatting, export wiring
- `readout-gui/src/widgets/csv_viewer/data_store.rs` — source registry, CSV/tail/runtime ingestion, mode filtering, hover resolution, export rows
- `readout-gui/src/widgets/csv_viewer/viewer_toolbar.rs` — `Attach Live` and `Tail CSV` menus, source chips keyed by stable source IDs
- `readout-gui/src/widgets/csv_viewer/info_bar.rs` — richer hover readout that can show wall-clock or sequence-index X labels
- `readout-gui/src/widgets/csv_viewer/overlay.rs` — keep markers/measure/select overlays, remove duplicate cursor tooltip
- `readout-gui/src/widgets/csv_viewer/mod.rs` tests — follow and action-level tests
- `readout-gui/src/widgets/csv_viewer/data_store.rs` tests — source registry, live tail, runtime history, export tests
- `readout-gui/src/app.rs` — forward runtime events into the viewer during the existing drain loop

### Boundary Decisions

- Do not change `RuntimeEvent` or `DeviceMeasurement`; runtime wall-clock alignment is derived inside the viewer from the first attached sample.
- Do not add split view in this plan.
- Do not move live CSV polling off the UI thread in this scope; keep the documented local-file assumption, but make tailing more responsive and less visually bursty.

---

## Chunk 1: Source Registry Foundation

### Task 1: Introduce source model, stable source IDs, and X-domain guards

**Files:**
- Create: `readout-gui/src/widgets/csv_viewer/source_model.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Test: `readout-gui/src/widgets/csv_viewer/data_store.rs`

- [ ] **Step 1: Write failing source-model tests**

In `readout-gui/src/widgets/csv_viewer/data_store.rs`, add tests for the two new invariants:

```rust
// Add a small `write_temp_csv(&str) -> PathBuf` helper in the test module.

#[test]
fn attach_runtime_source_is_deduplicated_per_device() {
    let mut store = CsvDataStore::new();

    let first = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
    let second = store.attach_runtime_device(DeviceId::Multimeter).unwrap();

    assert_eq!(first, second);
    assert_eq!(store.sources().len(), 1);
}

#[test]
fn attach_rejects_incompatible_x_domain() {
    let path = write_temp_csv(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
         not-a-time,Multimeter,1.0,V,DCV,false,false,false\n",
    );

    let mut store = CsvDataStore::new();
    store.load_csv_file(path, false).unwrap();

    let err = store.attach_runtime_device(DeviceId::Multimeter).unwrap_err();
    assert!(err.to_string().contains("incompatible time axis"));
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui attach_runtime_source_is_deduplicated_per_device -- --exact`

Run: `cargo test -p readout-gui attach_rejects_incompatible_x_domain -- --exact`

Expected: FAIL to compile because `attach_runtime_device()` / `sources()` / the new domain logic do not exist yet.

- [ ] **Step 3: Add the shared source model and stable-ID plumbing**

Create `readout-gui/src/widgets/csv_viewer/source_model.rs` with the core types:

```rust
pub type ViewerSourceId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XDomain {
    WallClock,
    SequenceIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerSourceKind {
    CsvFile { path: PathBuf },
    LiveCsvTail { device: DeviceId, path: PathBuf },
    RuntimeDevice { device: DeviceId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewerSample {
    pub x: f64,
    pub x_label: String,
    pub value: Option<f64>,
    pub device: String,
    pub unit: String,
    pub mode: String,
    pub is_overload: bool,
    pub is_open: bool,
    pub is_short: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Ready,
    Waiting(String),
    Error(String),
}
```

Then refactor `CsvDataStore` to hold:

```rust
pub struct CsvDataStore {
    sources: Vec<ViewerSource>,
    next_source_id: ViewerSourceId,
    next_color_idx: usize,
    active_domain: Option<XDomain>,
}
```

Rules to implement immediately:

- `ViewerSourceId` is stable and must not be used as a `Vec` index
- every new attach path validates its `XDomain` against `active_domain`
- `sources()` returns an immutable slice for the toolbar/tests
- `mod.rs` declares `mod source_model;`

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui attach_runtime_source_is_deduplicated_per_device -- --exact`

Run: `cargo test -p readout-gui attach_rejects_incompatible_x_domain -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/source_model.rs readout-gui/src/widgets/csv_viewer/data_store.rs readout-gui/src/widgets/csv_viewer/mod.rs
git commit -m "refactor(csv-viewer): add source registry foundation"
```

### Task 2: Port historical CSV loading and explicit live-tail attachment to the registry

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Test: `readout-gui/src/widgets/csv_viewer/data_store.rs`

- [ ] **Step 1: Write failing registry/tail tests**

Add tests that cover stable source removal and live-tail append behavior:

```rust
// Add a small `csv_with_value(timestamp, value) -> String` helper beside `write_temp_csv()`.

#[test]
fn remove_source_uses_stable_id_instead_of_vec_position() {
    let a = write_temp_csv(csv_with_value("2026-03-29T10:00:00Z", 1.0));
    let b = write_temp_csv(csv_with_value("2026-03-29T10:00:01Z", 2.0));

    let mut store = CsvDataStore::new();
    let source_a = store.load_csv_file(a, false).unwrap();
    let source_b = store.load_csv_file(b, false).unwrap();

    store.remove_source(source_a);

    let points = store.query_points(source_b, 32);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].1, 2.0);
}

#[test]
fn poll_live_csv_sources_keeps_incomplete_trailing_row_until_completed() {
    // Port the current partial-row test to `attach_live_csv()`
}

#[test]
fn live_tail_reload_after_truncation_restarts_from_file_start() {
    // Write a CSV, tail it, truncate/replace it, then verify the store reloads from the new file.
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui remove_source_uses_stable_id_instead_of_vec_position -- --exact`

Run: `cargo test -p readout-gui poll_live_csv_sources_keeps_incomplete_trailing_row_until_completed -- --exact`

Run: `cargo test -p readout-gui live_tail_reload_after_truncation_restarts_from_file_start -- --exact`

Expected: FAIL because the store still assumes file-index semantics and has no explicit `attach_live_csv()` path.

- [ ] **Step 3: Refactor the store API around explicit source attachment**

In `data_store.rs`, implement the new public API:

```rust
pub fn load_csv_file(&mut self, path: PathBuf, replace_existing: bool) -> Result<ViewerSourceId, CsvViewerError>;
pub fn attach_live_csv(&mut self, device: DeviceId, path: PathBuf) -> Result<ViewerSourceId, CsvViewerError>;
pub fn remove_source(&mut self, source_id: ViewerSourceId);
pub fn query_points(&self, source_id: ViewerSourceId, target_points: usize) -> Vec<DataPoint>;
pub fn latest_live_x(&self) -> Option<f64>;
```

Key implementation rules:

- `Open` becomes `load_csv_file(..., true)` and clears the registry before attaching the replacement source
- `Add` becomes `load_csv_file(..., false)`
- `attach_live_csv()` deduplicates by `(device, path)` and marks the source as live-capable
- `poll_live_files()` becomes `poll_live_sources()` and only scans `LiveCsvTail` sources
- truncation/rotation resets the read offset and reparses from the file start
- mode lists and mode-change markers are recomputed per source after each append/reload
- `active_domain` is cleared when the last source is removed

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui remove_source_uses_stable_id_instead_of_vec_position -- --exact`

Run: `cargo test -p readout-gui poll_live_csv_sources_keeps_incomplete_trailing_row_until_completed -- --exact`

Run: `cargo test -p readout-gui live_tail_reload_after_truncation_restarts_from_file_start -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/data_store.rs readout-gui/src/widgets/csv_viewer/mod.rs
git commit -m "refactor(csv-viewer): move csv loading and tailing to source registry"
```

---

## Chunk 2: Runtime Live Integration

### Task 3: Add runtime source buffering with viewer-local wall-clock anchoring

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/source_model.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Test: `readout-gui/src/widgets/csv_viewer/data_store.rs`

- [ ] **Step 1: Write failing runtime-buffer tests**

Add tests for runtime buffering and timestamp derivation:

```rust
// Add a `fake_measurement(device, value) -> DeviceMeasurement` test helper in this module.

#[test]
fn runtime_measurements_buffer_only_after_attach() {
    let mut store = CsvDataStore::new();
    let measurement = fake_measurement(DeviceId::Multimeter, 12.0);

    store.handle_runtime_event(&RuntimeEvent::Measurement {
        device: DeviceId::Multimeter,
        value: measurement.clone(),
    });
    assert!(store.latest_live_x().is_none());

    let source_id = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
    store.handle_runtime_event(&RuntimeEvent::Measurement {
        device: DeviceId::Multimeter,
        value: measurement,
    });

    assert_eq!(store.query_points(source_id, 32).len(), 1);
}

#[test]
fn runtime_export_uses_derived_wall_clock_timestamps() {
    // Attach runtime source, feed two measurements with different Instants,
    // export, and assert RFC3339 timestamps are increasing and non-empty.
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui runtime_measurements_buffer_only_after_attach -- --exact`

Run: `cargo test -p readout-gui runtime_export_uses_derived_wall_clock_timestamps -- --exact`

Expected: FAIL because runtime sources and event ingestion do not exist yet.

- [ ] **Step 3: Implement runtime attach and event ingestion**

Add runtime-specific fields to `ViewerSource`:

```rust
pub struct RuntimeAnchor {
    pub first_monotonic: Instant,
    pub first_wall_clock_epoch: f64,
}

pub struct ViewerSource {
    pub id: ViewerSourceId,
    pub kind: ViewerSourceKind,
    pub x_domain: XDomain,
    pub label: String,
    pub visible: bool,
    pub color: egui::Color32,
    pub status: SourceStatus,
    pub samples: Vec<ViewerSample>,
    pub mode_changes: Vec<usize>,
    pub runtime_anchor: Option<RuntimeAnchor>,
    pub last_read_pos: u64,
}
```

Then implement:

```rust
pub fn attach_runtime_device(&mut self, device: DeviceId) -> Result<ViewerSourceId, CsvViewerError>;
pub fn handle_runtime_event(&mut self, event: &RuntimeEvent);
pub fn export_rows(&self, selection: Option<(f64, f64)>) -> Vec<ExportRow>;
```

Runtime rules:

- runtime sources always use `XDomain::WallClock`
- attach creates the source in `SourceStatus::Waiting("Waiting for samples".into())`
- first measurement records the anchor and flips status to `Ready`
- later measurements derive wall-clock `x` from `anchor.first_wall_clock_epoch + (measurement.timestamp - anchor.first_monotonic).as_secs_f64()`
- `ConnectionChanged` updates runtime-source status to a waiting/disconnected message instead of silently doing nothing

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui runtime_measurements_buffer_only_after_attach -- --exact`

Run: `cargo test -p readout-gui runtime_export_uses_derived_wall_clock_timestamps -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/source_model.rs readout-gui/src/widgets/csv_viewer/data_store.rs
git commit -m "feat(csv-viewer): add runtime live source buffering"
```

### Task 4: Wire runtime events from `ReadOutApp` and add explicit attach actions in the viewer

**Files:**
- Modify: `readout-gui/src/app.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/viewer_toolbar.rs`
- Test: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Write failing viewer-action tests**

In `mod.rs`, add action-level tests for the new entry points:

```rust
#[test]
fn attach_live_csv_without_configured_path_sets_error() {
    let mut viewer = CsvViewerWindow::new();
    let config = AppConfiguration::default();

    viewer.handle_action(ViewerAction::AttachLiveCsv(DeviceId::Multimeter), &config);

    assert!(viewer.last_error.as_deref().unwrap().contains("CSV log path"));
}

#[test]
fn attach_runtime_action_creates_waiting_source() {
    let mut viewer = CsvViewerWindow::new();
    let config = AppConfiguration::default();

    viewer.handle_action(ViewerAction::AttachRuntime(DeviceId::Multimeter), &config);

    assert_eq!(viewer.data_store.sources().len(), 1);
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui attach_live_csv_without_configured_path_sets_error -- --exact`

Run: `cargo test -p readout-gui attach_runtime_action_creates_waiting_source -- --exact`

Expected: FAIL because these action variants and config-aware handlers do not exist yet.

- [ ] **Step 3: Expand toolbar actions and wire runtime forwarding**

In `viewer_toolbar.rs`, add the new menus and source-ID-based chip actions:

```rust
pub enum ViewerAction {
    None,
    OpenFile,
    AddFile,
    AttachRuntime(DeviceId),
    AttachLiveCsv(DeviceId),
    ZoomFit,
    SetMode(InteractionMode),
    Export,
    ToggleFollow,
    ToggleSourceVisibility(ViewerSourceId),
    RemoveSource(ViewerSourceId),
}
```

UI changes:

- `Attach Live ▾` menu with `Multimeter` and `USB-C`
- `Tail CSV ▾` menu with `Multimeter CSV` and `USB-C CSV`
- source chips use stable `source_id`, not loop index

In `mod.rs`:

- change `handle_action()` to accept `&AppConfiguration`
- add `configured_tail_path(config, device) -> Option<PathBuf>`
- map `AttachLiveCsv` to `data_store.attach_live_csv(...)`
- map `AttachRuntime` to `data_store.attach_runtime_device(...)`
- keep `Open` and `Add` backed by the refactored store API

In `app.rs`, forward runtime events into the viewer inside the existing drain loop:

```rust
while let Ok(event) = runtime.event_rx.try_recv() {
    self.csv_viewer.handle_runtime_event(&event);
    self.state.handle_event(event);
}
```

Also add a small `CsvViewerWindow` method that simply delegates:

```rust
pub fn handle_runtime_event(&mut self, event: &RuntimeEvent) {
    self.data_store.handle_runtime_event(event);
}
```

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui attach_live_csv_without_configured_path_sets_error -- --exact`

Run: `cargo test -p readout-gui attach_runtime_action_creates_waiting_source -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/app.rs readout-gui/src/widgets/csv_viewer/mod.rs readout-gui/src/widgets/csv_viewer/viewer_toolbar.rs
git commit -m "feat(csv-viewer): add explicit runtime and tail attach actions"
```

---

## Chunk 3: Viewer UX and Robustness

### Task 5: Fix follow semantics and make live updates smoother

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Test: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Write failing follow tests**

Add pure-helper tests in `mod.rs` so follow behavior is testable without GUI interaction:

```rust
#[test]
fn enabling_follow_requests_live_snap_even_when_latest_point_is_inside_bounds() {
    let next = compute_follow_window(100.0, 200.0, 150.0, true).unwrap();
    assert_eq!(next, (50.0, 150.0));
}

#[test]
fn follow_window_returns_none_without_positive_width() {
    assert!(compute_follow_window(100.0, 100.0, 150.0, true).is_none());
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui enabling_follow_requests_live_snap_even_when_latest_point_is_inside_bounds -- --exact`

Run: `cargo test -p readout-gui follow_window_returns_none_without_positive_width -- --exact`

Expected: FAIL because `compute_follow_window()` does not exist yet and follow is still tied to `latest_x > bounds.max()[0]`.

- [ ] **Step 3: Implement explicit snap-follow behavior and faster polling**

In `mod.rs`:

- add `const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(200);`
- replace the fixed `1 s` cadence with that constant
- add `snap_follow_next_frame: bool` to `CsvViewerWindow`
- when `ToggleFollow` turns follow on, set `snap_follow_next_frame = true`
- manual drag/scroll still disables follow

Implement a helper:

```rust
fn compute_follow_window(x_min: f64, x_max: f64, latest_x: f64, force_snap: bool) -> Option<(f64, f64)> {
    let width = x_max - x_min;
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    if !force_snap && latest_x <= x_max {
        return None;
    }
    Some((latest_x - width, latest_x))
}
```

Then use it from `follow_live_edge()` so re-enabling follow immediately snaps to the right edge even when the live point already sits inside the current bounds.

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui enabling_follow_requests_live_snap_even_when_latest_point_is_inside_bounds -- --exact`

Run: `cargo test -p readout-gui follow_window_returns_none_without_positive_width -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/mod.rs readout-gui/src/widgets/csv_viewer/data_store.rs
git commit -m "fix(csv-viewer): improve follow behavior and live polling cadence"
```

### Task 6: Unify hover resolution, remove duplicate tooltip systems, and export all source kinds

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/info_bar.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/overlay.rs`
- Test: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Test: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Write failing hover/export tests**

Add one hover test and one export test:

```rust
// Add a tiny `push_test_sample()` helper behind `#[cfg(test)]`, or construct test sources directly.

#[test]
fn nearest_visible_sample_prefers_matching_y_when_x_values_overlap() {
    let mut store = CsvDataStore::new();
    let low = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
    let high = store.attach_runtime_device(DeviceId::UsbC).unwrap();

    store.push_test_sample(low, 100.0, Some(1.0), "V", "DCV");
    store.push_test_sample(high, 100.0, Some(9.0), "V", "DCV");

    let hovered = store.nearest_visible_sample(100.0, 8.8).unwrap();
    assert_eq!(hovered.series, "USB-C Live");
    assert_eq!(hovered.value, 9.0);
}

#[test]
fn export_to_csv_includes_runtime_samples_and_selection_filter() {
    // Attach runtime source, feed two samples, export a narrow range, assert one row is written.
}
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo test -p readout-gui nearest_visible_sample_prefers_matching_y_when_x_values_overlap -- --exact`

Run: `cargo test -p readout-gui export_to_csv_includes_runtime_samples_and_selection_filter -- --exact`

Expected: FAIL because hover still resolves by X only and export still iterates raw `LoadedFile` rows.

- [ ] **Step 3: Replace dual tooltip logic with one resolved hover sample**

In `data_store.rs`:

- rename the hover helper to `nearest_visible_sample(x: f64, y: f64) -> Option<HoveredSample>`
- resolve the best sample by minimizing `(abs(sample.x - x), abs(sample.value.unwrap_or_default() - y))`
- return one struct used by both the info bar and overlay

In `mod.rs`:

- stop using `label_formatter()` for per-point readout; return an empty string there so `egui_plot` does not render a second tooltip
- keep `x_axis_formatter()`, but format from the active domain:
  - `WallClock` => `HH:MM:SS`
  - `SequenceIndex` => `#123`
- set `hovered_cursor` from `nearest_visible_sample(cursor_pos.x, cursor_pos.y)`
- update `export_to_csv()` to serialize `data_store.export_rows(selection)` instead of iterating old `LoadedFile` state

In `overlay.rs` and `info_bar.rs`:

- remove the raw `x = ... / y = ...` cursor bubble
- render one tooltip from the resolved hover sample
- show sequence-index labels cleanly in the info bar

- [ ] **Step 4: Run targeted tests to verify they pass**

Run: `cargo test -p readout-gui nearest_visible_sample_prefers_matching_y_when_x_values_overlap -- --exact`

Run: `cargo test -p readout-gui export_to_csv_includes_runtime_samples_and_selection_filter -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/mod.rs readout-gui/src/widgets/csv_viewer/data_store.rs readout-gui/src/widgets/csv_viewer/info_bar.rs readout-gui/src/widgets/csv_viewer/overlay.rs
git commit -m "fix(csv-viewer): unify hover tooltip and export across source kinds"
```

### Task 7: Full verification and smoke checklist

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/*` (only if verification finds issues)
- Modify: `readout-gui/src/app.rs` (only if verification finds issues)

- [ ] **Step 1: Run focused CSV Viewer tests**

Run: `cargo test -p readout-gui widgets::csv_viewer`

Expected: PASS

- [ ] **Step 2: Run touched-crate verification**

Run: `cargo test -p readout-gui -p readout-core -p readout-persistence`

Expected: PASS

- [ ] **Step 3: Run lint verification**

Run: `cargo clippy -p readout-gui -p readout-core -p readout-persistence -- -D warnings`

Expected: PASS

- [ ] **Step 4: Manual smoke test**

Open the app and verify:

- `Open` still replaces the source set with a historical CSV
- `Add` overlays another historical CSV
- `Attach Live -> Multimeter` starts an `MM Live` series only after live measurements arrive
- `Tail CSV -> Multimeter CSV` follows the configured file and updates noticeably more smoothly than the old `1 s` cadence
- enabling `Follow` snaps immediately to the right edge
- manual pan disables follow
- only one hover tooltip is visible
- runtime export writes the same CSV columns as file export

- [ ] **Step 5: Review hook artifacts and commit final polish**

After the verification commit, inspect:

- `~/Dev/code-review/reviews/readOutRS/unresolved.md`
- `~/Dev/code-review/prompts/readOutRS.suggestions.md`

If there are relevant new findings for these changes, fix them before the final commit.

Then commit:

```bash
git add readout-gui/src/app.rs readout-gui/src/widgets/csv_viewer
git commit -m "feat(csv-viewer): add hybrid runtime and csv live sources"
```
