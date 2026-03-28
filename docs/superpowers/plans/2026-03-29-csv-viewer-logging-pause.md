# CSV Viewer & Logging Pause Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an interactive CSV analysis window with full analytics tooling, and make recording indicators clickable to toggle CSV/OBS logging.

**Architecture:** Two independent features sharing one plan. Logging Pause extends existing config/UI with minimal changes. CSV Viewer is a new egui viewport window using egui_plot for chart rendering + custom overlay for analytics (measurements, markers, selection stats). Data layer parses CSV files and supports live tailing.

**Tech Stack:** Rust, egui/eframe, egui_plot, tokio (for async file I/O), rfd (file dialogs), serde

**Spec:** `docs/superpowers/specs/2026-03-29-csv-viewer-logging-pause-design.md`

---

## File Structure

### New Files
- `crates/readout-core/src/downsampling.rs` — extracted min-max and average downsampling algorithms (standalone functions on slices)
- `crates/readout-core/src/csv_record.rs` — `CsvRecord` struct + CSV parser
- `crates/readout-core/tests/downsampling_test.rs` — tests for extracted downsampling
- `crates/readout-core/tests/csv_record_test.rs` — tests for CSV parsing
- `readout-gui/src/widgets/csv_viewer/mod.rs` — `CsvViewerWindow` main struct + viewport
- `readout-gui/src/widgets/csv_viewer/data_store.rs` — `CsvDataStore` (file loading, querying, live polling)
- `readout-gui/src/widgets/csv_viewer/viewer_toolbar.rs` — viewer-internal toolbar (Open, Add, tools, export, live indicator)
- `readout-gui/src/widgets/csv_viewer/info_bar.rs` — bottom info bar (cursor, stats, deltas)
- `readout-gui/src/widgets/csv_viewer/overlay.rs` — interaction layer (crosshair, markers, measurements, selection)

### Modified Files
- `crates/readout-persistence/src/config.rs` — add `multimeter_obs_enabled`, `usbc_obs_enabled` fields
- `crates/readout-persistence/tests/config_test.rs` — test new fields
- `crates/readout-core/src/chart_pipeline.rs` — delegate to `downsampling.rs`
- `crates/readout-core/src/lib.rs` — export new modules
- `readout-gui/src/widgets/device_section.rs:64-78` — make recording icons clickable
- `readout-gui/src/widgets/toolbar.rs:14-26` — add `OpenCsvViewer` variant
- `readout-gui/src/widgets/toolbar.rs:82-96` — add CSV Viewer button
- `readout-gui/src/app.rs` — add CsvViewerWindow field, handle actions, viewport management
- `readout-gui/src/widgets/mod.rs` — export csv_viewer module

---

## Chunk 1: Logging Pause

### Task 1: Add OBS enabled fields to config

**Files:**
- Modify: `crates/readout-persistence/src/config.rs:153-165`
- Modify: `crates/readout-persistence/src/config.rs:265-272` (defaults)
- Modify: `crates/readout-persistence/src/config.rs:363-375` (inner struct)
- Modify: `crates/readout-persistence/src/config.rs:428-435` (inner → outer)
- Test: `crates/readout-persistence/tests/config_test.rs`

- [ ] **Step 1: Write test for new config fields**

In `crates/readout-persistence/tests/config_test.rs`, add:

```rust
#[test]
fn obs_enabled_defaults_to_true() {
    let config = AppConfiguration::default();
    assert!(config.multimeter_obs_enabled);
    assert!(config.usbc_obs_enabled);
}

#[test]
fn obs_enabled_round_trips_through_json() {
    let mut config = AppConfiguration::default();
    config.multimeter_obs_enabled = false;
    config.usbc_obs_enabled = false;
    let json = serde_json::to_string(&config).unwrap();
    let loaded: AppConfiguration = serde_json::from_str(&json).unwrap();
    assert!(!loaded.multimeter_obs_enabled);
    assert!(!loaded.usbc_obs_enabled);
}

#[test]
fn obs_enabled_missing_in_json_defaults_to_true() {
    // Simulate old config without the new fields
    let json = r#"{"multimeter_output_file": "test.txt"}"#;
    let config: AppConfiguration = serde_json::from_str(json).unwrap();
    assert!(config.multimeter_obs_enabled);
    assert!(config.usbc_obs_enabled);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-persistence obs_enabled`
Expected: compilation error — fields don't exist yet

- [ ] **Step 3: Add fields to AppConfiguration**

In `crates/readout-persistence/src/config.rs`, add after the OBS output section (after line 165):

```rust
#[serde(default = "default_true")]
pub multimeter_obs_enabled: bool,
#[serde(default = "default_true")]
pub usbc_obs_enabled: bool,
```

Add the default function (near other defaults):

```rust
fn default_true() -> bool {
    true
}
```

Add to `Default` impl (after line 272):

```rust
multimeter_obs_enabled: true,
usbc_obs_enabled: true,
```

Add to inner struct, inner→outer mapping, and `clamp_values()` if needed (mirror existing pattern for other bool fields).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p readout-persistence obs_enabled`
Expected: all 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/readout-persistence/src/config.rs crates/readout-persistence/tests/config_test.rs
git commit -m "feat(config): add multimeter_obs_enabled and usbc_obs_enabled fields"
```

### Task 2: Make recording indicators clickable

**Files:**
- Modify: `readout-gui/src/widgets/device_section.rs:58-79`
- Modify: `readout-gui/src/app.rs:486-495` (pass per-device active state + new obs_enabled logic)

- [ ] **Step 1: Define DeviceRecordingAction**

In `device_section.rs`, add a new action type (or extend existing `SectionAction`):

```rust
pub enum DeviceRecordingAction {
    None,
    ToggleCsvLogging(DeviceId),
    ToggleObsOutput(DeviceId),
}
```

- [ ] **Step 2: Replace label icons with clickable buttons**

In `device_section.rs`, replace the CSV indicator block (lines 64-70) from:

```rust
if csv_active {
    ui.label(
        RichText::new(egui_phosphor::regular::RECORD.to_string())
            .size(10.0)
            .color(crate::theme::colors::ERROR),
    ).on_hover_text("CSV logging");
}
```

To a clickable version that works for both active (red) and enabled-but-logging (toggleable):

```rust
if csv_configured {
    let icon_color = if csv_active {
        crate::theme::colors::ERROR  // red — recording
    } else {
        ui.visuals().weak_text_color()  // grey — disabled
    };
    let btn = ui.add(
        egui::Label::new(
            RichText::new(egui_phosphor::regular::RECORD.to_string())
                .size(10.0)
                .color(icon_color),
        ).sense(egui::Sense::click()),
    );
    if btn.clicked() {
        recording_action = DeviceRecordingAction::ToggleCsvLogging(device_id);
    }
    btn.on_hover_text(if csv_active { "CSV logging (click to stop)" } else { "CSV stopped (click to start)" });
}
```

Apply same pattern for OBS indicator (lines 72-78) with `ToggleObsOutput`.

- [ ] **Step 3: Update show() signature to pass device_id and per-device flags**

Change the `show()` function parameters: replace `csv_active: bool, obs_active: bool` with per-device booleans and device_id. Return `DeviceRecordingAction` alongside existing return types.

- [ ] **Step 4: Update app.rs to pass per-device state and handle actions**

In `app.rs`, update the call sites where `device_section.show()` is called:
- Pass `csv_configured` (file path non-empty), `csv_active` (enabled + path set), same for OBS (using new `*_obs_enabled` fields)
- Handle returned `DeviceRecordingAction`:

```rust
// Follow existing pattern: snapshot old config, mutate, compare, save, maybe restart
match recording_action {
    DeviceRecordingAction::ToggleCsvLogging(DeviceId::Multimeter) => {
        let old = self.config.clone();
        self.config.multimeter_csv_logging_enabled = !self.config.multimeter_csv_logging_enabled;
        self.enqueue_config_save(self.config.clone());
        if runtime_settings_changed(&old, &self.config) { self.restart_runtime(); }
    }
    DeviceRecordingAction::ToggleCsvLogging(DeviceId::UsbC) => {
        let old = self.config.clone();
        self.config.usbc_csv_logging_enabled = !self.config.usbc_csv_logging_enabled;
        self.enqueue_config_save(self.config.clone());
        if runtime_settings_changed(&old, &self.config) { self.restart_runtime(); }
    }
    DeviceRecordingAction::ToggleObsOutput(DeviceId::Multimeter) => {
        let old = self.config.clone();
        self.config.multimeter_obs_enabled = !self.config.multimeter_obs_enabled;
        self.enqueue_config_save(self.config.clone());
        if runtime_settings_changed(&old, &self.config) { self.restart_runtime(); }
    }
    DeviceRecordingAction::ToggleObsOutput(DeviceId::UsbC) => {
        let old = self.config.clone();
        self.config.usbc_obs_enabled = !self.config.usbc_obs_enabled;
        self.enqueue_config_save(self.config.clone());
        if runtime_settings_changed(&old, &self.config) { self.restart_runtime(); }
    }
    DeviceRecordingAction::None => {}
}
```

Note: `enqueue_config_save(config: AppConfiguration)` takes a config clone. `runtime_settings_changed(old, new)` is a free function, not a method.

- [ ] **Step 5: Add obs_enabled to runtime_settings_changed**

In `app.rs`, add the new fields to the free function `runtime_settings_changed()` (after line 323):

```rust
        || old.multimeter_obs_enabled != new.multimeter_obs_enabled
        || old.usbc_obs_enabled != new.usbc_obs_enabled
```

This ensures toggling OBS via the indicator actually triggers a runtime restart.

- [ ] **Step 6: Add OBS enabled checkboxes to settings panel**

In `readout-gui/src/widgets/settings.rs`, find the OBS settings section for each device (where output file picker and output mode are configured). Add a checkbox before the file picker:

```rust
ui.checkbox(&mut config.multimeter_obs_enabled, "OBS output enabled");
```

Same for USB-C. This ensures bidirectional sync between the indicator icon and settings panel (spec requirement).

- [ ] **Step 7: Update OBS active condition**

In `app.rs`, change OBS active logic from:
```rust
obs_active = !self.config.multimeter_output_file.is_empty()
```
To:
```rust
obs_active = self.config.multimeter_obs_enabled && !self.config.multimeter_output_file.is_empty()
```

Same for USB-C.

- [ ] **Step 8: Update runtime OBS logger startup**

In `app.rs` RuntimeHandle::start (lines 63-81), add `obs_enabled` check to OBS writer creation:

```rust
if config.multimeter_obs_enabled && !config.multimeter_output_file.is_empty() {
    // create ObsOutputWriter...
}
```

Same for USB-C.

- [ ] **Step 9: Build and verify**

Run: `cargo build -p readout-gui`
Expected: compiles without errors

- [ ] **Step 10: Commit**

```bash
git add readout-gui/src/widgets/device_section.rs readout-gui/src/widgets/settings.rs readout-gui/src/app.rs
git commit -m "feat(gui): clickable recording indicators to toggle CSV/OBS logging"
```

---

## Chunk 2: CSV Viewer — Data Layer

### Task 3: Extract downsampling algorithms

**Files:**
- Create: `crates/readout-core/src/downsampling.rs`
- Modify: `crates/readout-core/src/chart_pipeline.rs:182-226`
- Modify: `crates/readout-core/src/lib.rs`
- Create: `crates/readout-core/tests/downsampling_test.rs`

- [ ] **Step 1: Write tests for standalone downsampling**

Create `crates/readout-core/tests/downsampling_test.rs`:

```rust
use readout_core::downsampling::{min_max_downsample, average_downsample};
use std::time::Duration;

#[test]
fn min_max_preserves_peaks() {
    let samples: Vec<(Duration, f64)> = vec![
        (Duration::from_secs(0), 1.0),
        (Duration::from_secs(1), 5.0),  // max
        (Duration::from_secs(2), 0.5),  // min
        (Duration::from_secs(3), 3.0),
        (Duration::from_secs(4), 4.0),
        (Duration::from_secs(5), 2.0),
    ];
    let result = min_max_downsample(&samples, 4); // 2 buckets × 2 points
    let values: Vec<f64> = result.iter().map(|p| p.1).collect();
    assert!(values.contains(&5.0), "should preserve max");
    assert!(values.contains(&0.5), "should preserve min");
}

#[test]
fn min_max_returns_empty_for_empty_input() {
    let result = min_max_downsample(&[], 10);
    assert!(result.is_empty());
}

#[test]
fn min_max_returns_all_when_fewer_than_target() {
    let samples = vec![
        (Duration::from_secs(0), 1.0),
        (Duration::from_secs(1), 2.0),
    ];
    let result = min_max_downsample(&samples, 10);
    assert_eq!(result.len(), 2);
}

#[test]
fn average_downsample_reduces_count() {
    let samples: Vec<(Duration, f64)> = (0..100)
        .map(|i| (Duration::from_millis(i * 100), i as f64))
        .collect();
    let result = average_downsample(&samples, 10);
    assert!(result.len() <= 10);
    assert!(!result.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-core downsampling`
Expected: compilation error — module doesn't exist

- [ ] **Step 3: Create downsampling.rs with extracted algorithms**

Create `crates/readout-core/src/downsampling.rs`:

```rust
use std::time::Duration;

/// A point on a chart: (time offset, value).
pub type DataPoint = (Duration, f64);

/// Min-max downsampling: preserves peaks and valleys.
/// Returns up to `target_points` points (pairs of min/max per bucket).
pub fn min_max_downsample(samples: &[DataPoint], target_points: usize) -> Vec<DataPoint> {
    if samples.is_empty() || target_points == 0 {
        return Vec::new();
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

    for i in 0..bucket_count {
        let start = (i as f64 * bucket_size) as usize;
        let end = (((i + 1) as f64 * bucket_size) as usize).min(samples.len());
        if start >= end {
            continue;
        }

        let mut min_idx = start;
        let mut max_idx = start;
        for j in start..end {
            if samples[j].1.total_cmp(&samples[min_idx].1).is_lt() {
                min_idx = j;
            }
            if samples[j].1.total_cmp(&samples[max_idx].1).is_gt() {
                max_idx = j;
            }
        }

        // Add in chronological order, deduplicate if same index
        if min_idx == max_idx {
            result.push(samples[min_idx]);
        } else if min_idx < max_idx {
            result.push(samples[min_idx]);
            result.push(samples[max_idx]);
        } else {
            result.push(samples[max_idx]);
            result.push(samples[min_idx]);
        }
    }

    result
}

/// Average downsampling: smoother output for display.
/// Returns up to `target_points` averaged points.
pub fn average_downsample(samples: &[DataPoint], target_points: usize) -> Vec<DataPoint> {
    if samples.is_empty() || target_points == 0 {
        return Vec::new();
    }
    if samples.len() <= target_points {
        return samples.to_vec();
    }

    let bucket_size = samples.len() as f64 / target_points as f64;
    let mut result = Vec::with_capacity(target_points);

    for i in 0..target_points {
        let start = (i as f64 * bucket_size) as usize;
        let end = (((i + 1) as f64 * bucket_size) as usize).min(samples.len());
        if start >= end {
            continue;
        }

        let count = (end - start) as f64;
        let avg_time = {
            let sum: u128 = samples[start..end].iter().map(|s| s.0.as_nanos()).sum();
            Duration::from_nanos((sum as f64 / count) as u64)
        };
        let avg_value: f64 = samples[start..end].iter().map(|s| s.1).sum::<f64>() / count;
        result.push((avg_time, avg_value));
    }

    result
}
```

- [ ] **Step 4: Export module in lib.rs**

Add to `crates/readout-core/src/lib.rs`:

```rust
pub mod downsampling;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p readout-core downsampling`
Expected: all 4 tests PASS

- [ ] **Step 6: Refactor ChartPipeline to use extracted functions**

In `crates/readout-core/src/chart_pipeline.rs`:
- Keep `ChartPoint = (Duration, f64)` as the existing type alias (it's identical to `DataPoint`)
- Remove the inline `min_max_downsample` and `average_downsample` functions
- Replace with calls to `crate::downsampling::min_max_downsample` and `crate::downsampling::average_downsample`
- If `ChartPoint` and `DataPoint` are the same type `(Duration, f64)`, no conversion needed — just import and call directly. If they differ (e.g., `ChartPoint` uses `f64` for time), add a thin conversion at the call site.

- [ ] **Step 7: Run all existing chart_pipeline tests**

Run: `cargo test -p readout-core chart_pipeline`
Expected: all existing tests still PASS

- [ ] **Step 8: Commit**

```bash
git add crates/readout-core/src/downsampling.rs crates/readout-core/src/chart_pipeline.rs crates/readout-core/src/lib.rs crates/readout-core/tests/downsampling_test.rs
git commit -m "refactor(core): extract downsampling algorithms into standalone module"
```

### Task 4: CsvRecord struct and CSV parser

**Files:**
- Create: `crates/readout-core/src/csv_record.rs`
- Modify: `crates/readout-core/src/lib.rs`
- Create: `crates/readout-core/tests/csv_record_test.rs`

- [ ] **Step 1: Write tests for CSV parsing**

Create `crates/readout-core/tests/csv_record_test.rs`:

```rust
use readout_core::csv_record::{CsvRecord, parse_csv_file};

#[test]
fn parse_single_row() {
    let csv = "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
               2026-03-29T10:30:00.123,Multimeter,12.345,V DC,DCV,false,false,false\n";
    let records = parse_csv_file(csv.as_bytes()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].value, 12.345);
    assert_eq!(records[0].unit, "V DC");
    assert_eq!(records[0].mode, "DCV");
    assert!(!records[0].is_overload);
}

#[test]
fn parse_multiple_rows() {
    let csv = "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
               2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n\
               2026-03-29T10:30:01,Multimeter,12.346,V DC,DCV,false,false,false\n";
    let records = parse_csv_file(csv.as_bytes()).unwrap();
    assert_eq!(records.len(), 2);
}

#[test]
fn parse_handles_mode_change() {
    let csv = "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
               2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n\
               2026-03-29T10:30:01,Multimeter,0.001,A DC,DCA,false,false,false\n";
    let records = parse_csv_file(csv.as_bytes()).unwrap();
    assert_eq!(records[0].mode, "DCV");
    assert_eq!(records[1].mode, "DCA");
}

#[test]
fn parse_skips_malformed_rows() {
    let csv = "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
               bad_row\n\
               2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n";
    let records = parse_csv_file(csv.as_bytes()).unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn parse_empty_file_returns_empty() {
    let csv = "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n";
    let records = parse_csv_file(csv.as_bytes()).unwrap();
    assert!(records.is_empty());
}

#[test]
fn mode_change_indices_detected() {
    let csv = "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
               2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n\
               2026-03-29T10:30:01,Multimeter,12.346,V DC,DCV,false,false,false\n\
               2026-03-29T10:30:02,Multimeter,0.001,A DC,DCA,false,false,false\n";
    let records = parse_csv_file(csv.as_bytes()).unwrap();
    let changes: Vec<usize> = readout_core::csv_record::find_mode_changes(&records);
    assert_eq!(changes, vec![2]); // index 2 is where DCA starts
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-core csv_record`
Expected: compilation error

- [ ] **Step 3: Implement CsvRecord and parser**

Create `crates/readout-core/src/csv_record.rs`:

```rust
use std::io::BufRead;

#[derive(Debug, Clone)]
pub struct CsvRecord {
    pub timestamp: String,
    pub device: String,
    pub value: f64,
    pub unit: String,
    pub mode: String,
    pub is_overload: bool,
    pub is_open: bool,
    pub is_short: bool,
}

/// Parse CSV file content into records. Skips header and malformed rows.
pub fn parse_csv_file(reader: impl BufRead) -> Result<Vec<CsvRecord>, std::io::Error> {
    let mut records = Vec::new();
    let mut lines = reader.lines();

    // Skip header
    if lines.next().is_none() {
        return Ok(records);
    }

    for line in lines {
        let line = line?;
        if let Some(record) = parse_row(&line) {
            records.push(record);
        }
    }

    Ok(records)
}

pub fn parse_row(line: &str) -> Option<CsvRecord> {
    let fields: Vec<&str> = line.splitn(8, ',').collect();
    if fields.len() < 8 {
        return None;
    }

    Some(CsvRecord {
        timestamp: fields[0].to_string(),
        device: fields[1].to_string(),
        value: fields[2].parse().ok()?,
        unit: fields[3].to_string(),
        mode: fields[4].to_string(),
        is_overload: fields[5].parse().unwrap_or(false),
        is_open: fields[6].parse().unwrap_or(false),
        is_short: fields[7].parse().unwrap_or(false),
    })
}

/// Find indices where mode changes (compared to previous record).
pub fn find_mode_changes(records: &[CsvRecord]) -> Vec<usize> {
    let mut changes = Vec::new();
    for i in 1..records.len() {
        if records[i].mode != records[i - 1].mode {
            changes.push(i);
        }
    }
    changes
}
```

- [ ] **Step 4: Export module in lib.rs**

Add to `crates/readout-core/src/lib.rs`:

```rust
pub mod csv_record;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p readout-core csv_record`
Expected: all 6 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/readout-core/src/csv_record.rs crates/readout-core/src/lib.rs crates/readout-core/tests/csv_record_test.rs
git commit -m "feat(core): add CsvRecord struct and CSV parser with mode change detection"
```

### Task 5: CsvDataStore

**Files:**
- Create: `readout-gui/src/widgets/csv_viewer/data_store.rs`

- [ ] **Step 1: Implement CsvDataStore**

```rust
use readout_core::csv_record::{CsvRecord, parse_csv_file, find_mode_changes};
use readout_core::downsampling::{min_max_downsample, DataPoint};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

/// Unique identifier for a loaded file.
pub type FileId = usize;

/// Color palette for overlay files.
const COLORS: &[[u8; 3]] = &[
    [74, 158, 255],   // blue
    [78, 205, 196],   // teal
    [255, 107, 107],  // red
    [255, 217, 61],   // yellow
    [168, 120, 255],  // purple
];

pub struct LoadedFile {
    pub path: PathBuf,
    pub records: Vec<CsvRecord>,
    pub mode_changes: Vec<usize>,
    pub visible: bool,
    pub color: egui::Color32,
    /// Unique modes found in this file.
    pub modes: Vec<String>,
    /// Which modes are currently visible (filter).
    pub visible_modes: std::collections::HashSet<String>,
    /// For live polling: byte offset of last read position.
    pub last_read_pos: u64,
    /// Whether this file is being actively recorded to.
    pub is_live: bool,
}

pub struct CsvDataStore {
    files: Vec<LoadedFile>,
    next_id: FileId,
}

impl CsvDataStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            next_id: 0,
        }
    }

    /// Load a CSV file. Returns the FileId.
    pub fn load_file(&mut self, path: PathBuf, is_live: bool) -> Result<FileId, std::io::Error> {
        let content = std::fs::read(&path)?;
        let records = parse_csv_file(BufReader::new(content.as_slice()))?;
        let mode_changes = find_mode_changes(&records);
        let modes: Vec<String> = {
            let mut m: Vec<String> = records.iter().map(|r| r.mode.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
            m.sort();
            m
        };
        let visible_modes = modes.iter().cloned().collect();
        let color_idx = self.next_id % COLORS.len();
        let color = egui::Color32::from_rgb(COLORS[color_idx][0], COLORS[color_idx][1], COLORS[color_idx][2]);

        let id = self.next_id;
        self.files.push(LoadedFile {
            path,
            records,
            mode_changes,
            visible: true,
            color,
            modes,
            visible_modes,
            last_read_pos: content.len() as u64,
            is_live,
        });
        self.next_id += 1;
        Ok(id)
    }

    /// Poll for new lines in live files.
    pub fn poll_live_files(&mut self) {
        for file in &mut self.files {
            if !file.is_live {
                continue;
            }
            if let Ok(mut f) = std::fs::File::open(&file.path) {
                let metadata = f.metadata().ok();
                let file_len = metadata.map(|m| m.len()).unwrap_or(0);

                // File truncated — reload from start
                if file_len < file.last_read_pos {
                    file.last_read_pos = 0;
                    file.records.clear();
                    file.mode_changes.clear();
                }

                if file_len > file.last_read_pos {
                    use std::io::Seek;
                    let _ = f.seek(SeekFrom::Start(file.last_read_pos));
                    let reader = BufReader::new(&f);
                    let old_len = file.records.len();
                    for line in reader.lines().flatten() {
                        if let Some(record) = readout_core::csv_record::parse_row(&line) {
                            file.records.push(record);
                        }
                    }
                    // Update mode changes for new records
                    if file.records.len() > old_len && old_len > 0 {
                        for i in old_len..file.records.len() {
                            if file.records[i].mode != file.records[i - 1].mode {
                                file.mode_changes.push(i);
                            }
                        }
                    }
                    // Update modes set
                    for r in &file.records[old_len..] {
                        if !file.modes.contains(&r.mode) {
                            file.modes.push(r.mode.clone());
                            file.visible_modes.insert(r.mode.clone());
                        }
                    }
                    file.last_read_pos = file_len;
                }
            }
        }
    }

    /// Get data points for a file, filtered by visible modes, downsampled for display.
    /// Note: X axis uses record index (0, 1, 2...) as seconds until Task 14 adds real timestamp parsing.
    /// This means the chart will show evenly-spaced points regardless of actual sample timing.
    /// Task 14 (Chunk 5) replaces this with parsed timestamps from the CSV.
    pub fn query_points(&self, file_id: FileId, target_points: usize) -> Vec<DataPoint> {
        let Some(file) = self.files.get(file_id) else {
            return Vec::new();
        };
        let points: Vec<DataPoint> = file.records.iter()
            .enumerate()
            .filter(|(_, r)| file.visible_modes.contains(&r.mode))
            .map(|(i, r)| (Duration::from_secs(i as u64), r.value))
            .collect();

        min_max_downsample(&points, target_points)
    }

    pub fn files(&self) -> &[LoadedFile] {
        &self.files
    }

    pub fn files_mut(&mut self) -> &mut Vec<LoadedFile> {
        &mut self.files
    }

    pub fn remove_file(&mut self, file_id: FileId) {
        if file_id < self.files.len() {
            self.files.remove(file_id);
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}
```

Note: `parse_row` was already made `pub` in Task 4 Step 3. X axis uses record index as placeholder until Task 14 adds real timestamp parsing.

- [ ] **Step 2: Build core crate to verify data_store compiles in isolation**

Run: `cargo check -p readout-core`
Expected: compiles. Note: `data_store.rs` won't be compiled yet — it's included when `mod.rs` is created in Task 6. This step only verifies core crate changes don't break.

- [ ] **Step 3: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/data_store.rs crates/readout-core/src/csv_record.rs
git commit -m "feat(gui): add CsvDataStore for loading and polling CSV files"
```

---

## Chunk 3: CSV Viewer — Window & Basic Chart

**Prerequisite:** Verify `rfd` crate is in `readout-gui/Cargo.toml`. If missing, add `rfd = "0.15"` to `[dependencies]` before proceeding. The CSV Viewer uses `rfd::FileDialog` for file open/save dialogs.

### Task 6: CSV Viewer window skeleton

**Files:**
- Create: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Create: `readout-gui/src/widgets/csv_viewer/viewer_toolbar.rs`
- Create: `readout-gui/src/widgets/csv_viewer/info_bar.rs`
- Modify: `readout-gui/src/widgets/mod.rs`

- [ ] **Step 1: Create mod.rs with CsvViewerWindow**

Create `readout-gui/src/widgets/csv_viewer/mod.rs`:

```rust
mod data_store;
mod viewer_toolbar;
mod info_bar;

use data_store::{CsvDataStore, FileId};
use egui_plot::{Plot, Line, PlotPoints};

pub struct CsvViewerWindow {
    pub open: bool,
    data_store: CsvDataStore,
    /// Currently active interaction mode.
    interaction_mode: InteractionMode,
    /// Whether auto-follow is active (live mode).
    following: bool,
    /// Pending file to open (from main window).
    pending_file: Option<std::path::PathBuf>,
    /// Timer for live polling.
    last_poll: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionMode {
    Normal,
    Measure,
    Select,
    Marker,
}

impl Default for InteractionMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Actions returned from the viewer toolbar.
pub enum ViewerAction {
    None,
    OpenFile,
    AddFile,
    ZoomFit,
    SetMode(InteractionMode),
    Export,
    ToggleFollow,
}

impl CsvViewerWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            data_store: CsvDataStore::new(),
            interaction_mode: InteractionMode::Normal,
            following: true,
            pending_file: None,
            last_poll: std::time::Instant::now(),
        }
    }

    /// Show the CSV Viewer as a separate viewport.
    /// IMPORTANT: The viewport closure captures `&mut self`. To avoid nested borrow issues,
    /// do NOT call `self.method()` inside the closure. Instead, access fields directly
    /// or extract data before the closure. Follow the same pattern as settings/meter_control viewports.
    pub fn show(&mut self, ctx: &egui::Context, config: &readout_persistence::AppConfiguration) {
        if !self.open {
            return;
        }

        let mut vp = egui::ViewportBuilder::default()
            .with_title("CSV Viewer")
            .with_inner_size([800.0, 500.0])
            .with_min_inner_size([400.0, 300.0]);
        if config.always_on_top {
            vp = vp.with_always_on_top();
        }

        // Collect config data needed inside closure to avoid borrow issues
        let is_live_mm = config.multimeter_csv_logging_enabled
            && !config.multimeter_csv_log_file_path.is_empty();
        let live_mm_path = config.multimeter_csv_log_file_path.clone();
        let is_live_usbc = config.usbc_csv_logging_enabled
            && !config.usbc_csv_log_file_path.is_empty();
        let live_usbc_path = config.usbc_csv_log_file_path.clone();

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("csv_viewer"),
            vp,
            |ctx, _class| {
                // Poll live files every 1s
                if self.last_poll.elapsed() >= std::time::Duration::from_secs(1) {
                    self.data_store.poll_live_files();
                    self.last_poll = std::time::Instant::now();
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    // Toolbar
                    let action = viewer_toolbar::show(
                        ui,
                        &self.data_store,
                        self.interaction_mode,
                        self.following,
                    );

                    // Handle toolbar action inline (no self.method() call to avoid borrow issues)
                    match action {
                        ViewerAction::OpenFile => {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("CSV", &["csv"])
                                .pick_file()
                            {
                                let path_str = path.to_string_lossy().to_string();
                                let is_live = (is_live_mm && path_str == live_mm_path)
                                    || (is_live_usbc && path_str == live_usbc_path);
                                self.data_store = CsvDataStore::new();
                                let _ = self.data_store.load_file(path, is_live);
                                if is_live { self.following = true; }
                            }
                        }
                        ViewerAction::AddFile => {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("CSV", &["csv"])
                                .pick_file()
                            {
                                let path_str = path.to_string_lossy().to_string();
                                let is_live = (is_live_mm && path_str == live_mm_path)
                                    || (is_live_usbc && path_str == live_usbc_path);
                                let _ = self.data_store.load_file(path, is_live);
                            }
                        }
                        ViewerAction::SetMode(mode) => self.interaction_mode = mode,
                        ViewerAction::ToggleFollow => self.following = !self.following,
                        ViewerAction::ZoomFit | ViewerAction::Export | ViewerAction::None => {}
                    }

                    ui.separator();

                    // Chart area (inline, not via self.show_chart())
                    self.render_chart(ui);

                    // Info bar
                    ui.separator();
                    info_bar::show(ui, None, None, None);
                });

                if ctx.input(|i| i.viewport().close_requested()) {
                    self.open = false;
                }
            },
        );
    }

    /// Render the chart. Called from within the viewport closure.
    /// Uses `&mut self` because overlay state is updated during rendering.
    fn render_chart(&mut self, ui: &mut egui::Ui) {
        let plot = Plot::new("csv_viewer_plot")
            .allow_zoom(true)
            .allow_drag(true)
            .allow_scroll(true)
            .show_axes(true)
            .show_grid(true)
            .legend(egui_plot::Legend::default());

        plot.show(ui, |plot_ui| {
            for (idx, file) in self.data_store.files().iter().enumerate() {
                if !file.visible {
                    continue;
                }
                let points = self.data_store.query_points(idx, 2000);
                let plot_points: PlotPoints = points
                    .iter()
                    .map(|(t, v)| [t.as_secs_f64(), *v])
                    .collect();
                let name = file.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("File {}", idx));
                plot_ui.line(
                    Line::new(plot_points)
                        .name(&name)
                        .color(file.color),
                );
            }
        });
    }

    // Note: handle_viewer_action is inlined in show() to avoid borrow issues.
    // Export action is handled in Task 13 by adding a branch to the match in show().
}
```

- [ ] **Step 2: Create viewer_toolbar.rs**

Create `readout-gui/src/widgets/csv_viewer/viewer_toolbar.rs`:

```rust
use super::{InteractionMode, ViewerAction};
use super::data_store::CsvDataStore;

pub fn show(
    ui: &mut egui::Ui,
    data_store: &CsvDataStore,
    current_mode: InteractionMode,
    following: bool,
) -> ViewerAction {
    let mut action = ViewerAction::None;

    ui.horizontal(|ui| {
        // Use egui_phosphor icons for consistency with the rest of the app
        if ui.button(RichText::new(format!("{} Open", egui_phosphor::regular::FOLDER_OPEN)).small()).clicked() {
            action = ViewerAction::OpenFile;
        }
        if ui.button(RichText::new(format!("{} Add", egui_phosphor::regular::PLUS)).small()).clicked() {
            action = ViewerAction::AddFile;
        }

        ui.separator();

        if ui.button(RichText::new(format!("{} Fit", egui_phosphor::regular::ARROWS_OUT)).small()).clicked() {
            action = ViewerAction::ZoomFit;
        }

        let mode_btn = |ui: &mut egui::Ui, label: &str, mode: InteractionMode, current: InteractionMode| -> bool {
            ui.selectable_label(current == mode, label).clicked()
        };

        if mode_btn(ui, "📏 Measure", InteractionMode::Measure, current_mode) {
            action = ViewerAction::SetMode(if current_mode == InteractionMode::Measure {
                InteractionMode::Normal
            } else {
                InteractionMode::Measure
            });
        }
        if mode_btn(ui, "▬ Select", InteractionMode::Select, current_mode) {
            action = ViewerAction::SetMode(if current_mode == InteractionMode::Select {
                InteractionMode::Normal
            } else {
                InteractionMode::Select
            });
        }
        if mode_btn(ui, "📌 Marker", InteractionMode::Marker, current_mode) {
            action = ViewerAction::SetMode(if current_mode == InteractionMode::Marker {
                InteractionMode::Normal
            } else {
                InteractionMode::Marker
            });
        }

        ui.separator();

        if ui.button(RichText::new(format!("{} Export", egui_phosphor::regular::EXPORT)).small()).clicked() {
            action = ViewerAction::Export;
        }

        // File legend chips
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Live indicator (clickable button to toggle follow)
            let has_live = data_store.files().iter().any(|f| f.is_live);
            if has_live {
                let label = if following { "🟢 Following" } else { "🟡 Paused" };
                if ui.button(egui::RichText::new(label).small()).clicked() {
                    action = ViewerAction::ToggleFollow;
                }
            }

            // File chips
            for file in data_store.files().iter().rev() {
                let name = file.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                ui.label(egui::RichText::new(format!("● {name}")).color(file.color).small());
            }
        });
    });

    action
}
```

- [ ] **Step 3: Create info_bar.rs**

Create `readout-gui/src/widgets/csv_viewer/info_bar.rs`:

```rust
pub struct CursorInfo {
    pub value: f64,
    pub unit: String,
    pub timestamp: String,
}

pub struct SelectionStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub stddev: f64,
}

pub struct MeasurementDelta {
    pub dt: String,
    pub dv: f64,
}

pub fn show(
    ui: &mut egui::Ui,
    cursor: Option<&CursorInfo>,
    stats: Option<&SelectionStats>,
    delta: Option<&MeasurementDelta>,
) {
    ui.horizontal(|ui| {
        // Cursor position
        if let Some(c) = cursor {
            ui.label(
                egui::RichText::new(format!("🖱️ {:.4} {} @ {}", c.value, c.unit, c.timestamp))
                    .small(),
            );
        } else {
            ui.label(egui::RichText::new("🖱️ —").small().weak());
        }

        ui.separator();

        // Selection stats
        if let Some(s) = stats {
            ui.label(egui::RichText::new(format!(
                "Min: {:.4}  Max: {:.4}  Avg: {:.4}  σ: {:.4}",
                s.min, s.max, s.avg, s.stddev
            )).small());
        }

        // Measurement delta
        if let Some(d) = delta {
            ui.separator();
            ui.label(egui::RichText::new(format!(
                "Δt: {}  Δv: {:.4}",
                d.dt, d.dv
            )).small());
        }
    });
}
```

- [ ] **Step 4: Export csv_viewer module**

In `readout-gui/src/widgets/mod.rs`, add:

```rust
pub mod csv_viewer;
```

- [ ] **Step 5: Build to verify compilation**

Run: `cargo build -p readout-gui`
Expected: compiles without errors

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/
git commit -m "feat(gui): add CSV Viewer window skeleton with toolbar and info bar"
```

### Task 7: Integrate CSV Viewer into app

**Files:**
- Modify: `readout-gui/src/widgets/toolbar.rs:14-26` (add OpenCsvViewer variant)
- Modify: `readout-gui/src/widgets/toolbar.rs:82-96` (add button)
- Modify: `readout-gui/src/app.rs` (add field, handle action, show viewport)

- [ ] **Step 1: Add OpenCsvViewer to ToolbarAction**

In `readout-gui/src/widgets/toolbar.rs`, add variant:

```rust
pub enum ToolbarAction {
    // ... existing variants ...
    OpenCsvViewer,
}
```

- [ ] **Step 2: Add CSV Viewer button to toolbar**

In `show_title_bar()`, add a new button (before or after the settings gear):

```rust
if ui.add(egui::Button::new(
    RichText::new(egui_phosphor::regular::CHART_LINE.to_string()).size(14.0)
).frame(false))
.on_hover_text("CSV Viewer (Cmd+L)")
.clicked() {
    action = ToolbarAction::OpenCsvViewer;
}
```

- [ ] **Step 3: Add CsvViewerWindow to ReadOutApp**

In `app.rs`, add field to `ReadOutApp`:

```rust
csv_viewer: widgets::csv_viewer::CsvViewerWindow,
```

Initialize in `new()`:

```rust
csv_viewer: widgets::csv_viewer::CsvViewerWindow::new(),
```

- [ ] **Step 4: Handle ToolbarAction::OpenCsvViewer**

In the toolbar action match block:

```rust
ToolbarAction::OpenCsvViewer => {
    self.csv_viewer.open = true;
}
```

- [ ] **Step 5: Show CSV Viewer viewport in update()**

In `update()`, after other viewport shows (settings, meter_control):

```rust
self.csv_viewer.show(ctx, &self.config);
```

- [ ] **Step 6: Add Cmd+L keyboard shortcut**

In the keyboard shortcut handling section of app.rs:

```rust
if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::L)) {
    self.csv_viewer.open = true;
}
```

- [ ] **Step 7: Build and test manually**

Run: `cargo build -p readout-gui`
Expected: compiles. Run the app, verify CSV Viewer button appears in toolbar, clicking opens empty viewer window with toolbar and info bar.

- [ ] **Step 8: Commit**

```bash
git add readout-gui/src/widgets/toolbar.rs readout-gui/src/app.rs
git commit -m "feat(gui): integrate CSV Viewer window with toolbar button and Cmd+L shortcut"
```

---

## Chunk 4: CSV Viewer — Analytics Overlay

### Task 8: Crosshair and tooltip

**Files:**
- Create: `readout-gui/src/widgets/csv_viewer/overlay.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Create overlay.rs with crosshair + tooltip state**

```rust
use egui_plot::PlotResponse;

/// State for all interactive overlays.
pub struct OverlayState {
    /// Last known cursor position on chart (in plot coordinates).
    pub cursor_pos: Option<egui_plot::PlotPoint>,
    /// Active measurements (pairs of points).
    pub measurements: Vec<Measurement>,
    /// Measurement in progress (first point placed).
    pub measuring_from: Option<egui_plot::PlotPoint>,
    /// Selection range (x_min, x_max).
    pub selection: Option<(f64, f64)>,
    /// Selection in progress (start x).
    pub selecting_from: Option<f64>,
    /// User-placed markers.
    pub markers: Vec<UserMarker>,
    /// Index of marker being edited (rename/delete popup).
    pub editing_marker: Option<usize>,
}

pub struct Measurement {
    pub from: egui_plot::PlotPoint,
    pub to: egui_plot::PlotPoint,
}

pub struct UserMarker {
    pub x: f64,
    pub label: String,
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            cursor_pos: None,
            measurements: Vec::new(),
            measuring_from: None,
            selection: None,
            selecting_from: None,
            markers: Vec::new(),
            editing_marker: None,
        }
    }
}

/// Show marker edit popup (rename or delete). Call from render_chart when editing_marker is Some.
pub fn show_marker_edit_popup(ui: &mut egui::Ui, state: &mut OverlayState) {
    if let Some(idx) = state.editing_marker {
        if idx < state.markers.len() {
            let marker = &mut state.markers[idx];
            egui::Window::new("Edit Marker")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Label:");
                        ui.text_edit_singleline(&mut marker.label);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Done").clicked() {
                            state.editing_marker = None;
                        }
                        if ui.button("Delete").clicked() {
                            state.markers.remove(idx);
                            state.editing_marker = None;
                        }
                    });
                });
        } else {
            state.editing_marker = None;
        }
    }
}
```

- [ ] **Step 2: Integrate overlay state into CsvViewerWindow**

Add `overlay: overlay::OverlayState` field to `CsvViewerWindow`. Initialize in `new()`.

Update `render_chart()` (signature already `&mut self` from Task 6 fix) to capture `PlotResponse` and update cursor position:

```rust
let response = plot.show(ui, |plot_ui| {
    // ... existing line drawing ...
    // Store pointer position
    if let Some(pos) = plot_ui.pointer_coordinate() {
        self.overlay.cursor_pos = Some(pos);
    } else {
        self.overlay.cursor_pos = None;
    }
});
```

Note: `render_chart` is `&mut self`, so mutating `self.overlay` inside `plot.show()` closure is valid since the closure captures `&mut self`.

- [ ] **Step 3: Update info_bar to show cursor position**

Pass `overlay.cursor_pos` to `info_bar::show()` as `CursorInfo`.

- [ ] **Step 4: Build and verify**

Run: `cargo build -p readout-gui`
Expected: compiles. Hovering over chart shows cursor position in info bar.

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/overlay.rs readout-gui/src/widgets/csv_viewer/mod.rs readout-gui/src/widgets/csv_viewer/info_bar.rs
git commit -m "feat(gui): add crosshair overlay and cursor position display in CSV Viewer"
```

### Task 9: Measurement tool

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/overlay.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Add measurement interaction logic**

In `overlay.rs`, add function:

```rust
pub fn handle_measure_interaction(
    state: &mut OverlayState,
    response: &egui::Response,
    cursor_pos: Option<egui_plot::PlotPoint>,
) {
    let Some(pos) = cursor_pos else { return };

    if response.clicked() {
        if let Some(from) = state.measuring_from.take() {
            // Second click: complete measurement
            state.measurements.push(Measurement { from, to: pos });
        } else {
            // First click: start measurement
            state.measuring_from = Some(pos);
        }
    }

    // Esc cancels current or removes last measurement
    if response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if state.measuring_from.is_some() {
            state.measuring_from = None;
        } else {
            state.measurements.pop();
        }
    }
}
```

- [ ] **Step 2: Draw measurement lines on chart**

Add drawing function called inside `plot.show()` closure:

```rust
pub fn draw_measurements(plot_ui: &mut egui_plot::PlotUi, state: &OverlayState, measuring_cursor: Option<egui_plot::PlotPoint>) {
    for m in &state.measurements {
        plot_ui.line(
            egui_plot::Line::new(vec![[m.from.x, m.from.y], [m.to.x, m.to.y]])
                .color(egui::Color32::YELLOW)
                .width(1.5),
        );
    }
    // In-progress measurement (from → cursor)
    if let (Some(from), Some(to)) = (&state.measuring_from, measuring_cursor) {
        plot_ui.line(
            egui_plot::Line::new(vec![[from.x, from.y], [to.x, to.y]])
                .color(egui::Color32::YELLOW.linear_multiply(0.6))
                .width(1.0)
                .style(egui_plot::LineStyle::dashed_dense()),
        );
    }
}
```

- [ ] **Step 3: Pass measurement delta to info_bar**

Compute Δt and Δv from the last measurement and pass to `info_bar::show()`.

- [ ] **Step 4: Wire into show_chart() based on interaction_mode**

In `mod.rs`, after `plot.show()`, if `interaction_mode == Measure`, call `handle_measure_interaction`.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p readout-gui`
Expected: compiles. In Measure mode, clicking places points and draws lines.

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/
git commit -m "feat(gui): add measurement tool to CSV Viewer"
```

### Task 10: Selection tool with stats

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/overlay.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/info_bar.rs`

- [ ] **Step 1: Add selection interaction logic**

In `overlay.rs`:

```rust
pub fn handle_select_interaction(
    state: &mut OverlayState,
    response: &egui::Response,
    cursor_pos: Option<egui_plot::PlotPoint>,
) {
    let Some(pos) = cursor_pos else { return };

    if response.drag_started() {
        state.selecting_from = Some(pos.x);
    }
    if response.dragged() {
        if let Some(from_x) = state.selecting_from {
            let x_min = from_x.min(pos.x);
            let x_max = from_x.max(pos.x);
            state.selection = Some((x_min, x_max));
        }
    }
    if response.drag_stopped() {
        state.selecting_from = None;
        // selection remains set
    }
    if response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.selection = None;
        state.selecting_from = None;
    }
}
```

- [ ] **Step 2: Add stats computation**

```rust
pub fn compute_selection_stats(records: &[readout_core::csv_record::CsvRecord], x_min: f64, x_max: f64) -> Option<super::info_bar::SelectionStats> {
    // Filter records within x range (using record index as x for now)
    let values: Vec<f64> = records.iter()
        .enumerate()
        .filter(|(i, _)| {
            let x = *i as f64;
            x >= x_min && x <= x_max
        })
        .map(|(_, r)| r.value)
        .collect();

    if values.is_empty() {
        return None;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();

    Some(super::info_bar::SelectionStats { min, max, avg, stddev })
}
```

- [ ] **Step 3: Draw selection highlight on chart**

```rust
pub fn draw_selection(plot_ui: &mut egui_plot::PlotUi, selection: Option<(f64, f64)>) {
    if let Some((x_min, x_max)) = selection {
        // Draw vertical lines at selection boundaries
        let bounds = plot_ui.plot_bounds();
        plot_ui.line(
            egui_plot::Line::new(vec![[x_min, bounds.min()[1]], [x_min, bounds.max()[1]]])
                .color(egui::Color32::from_rgba_premultiplied(100, 150, 255, 80))
                .width(1.0),
        );
        plot_ui.line(
            egui_plot::Line::new(vec![[x_max, bounds.min()[1]], [x_max, bounds.max()[1]]])
                .color(egui::Color32::from_rgba_premultiplied(100, 150, 255, 80))
                .width(1.0),
        );
    }
}
```

- [ ] **Step 4: Wire stats into info_bar**

Compute stats when selection is active and pass to `info_bar::show()`.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p readout-gui`

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/
git commit -m "feat(gui): add selection tool with statistics in CSV Viewer"
```

### Task 11: Marker tool and mode change markers

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/overlay.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Add marker interaction logic**

```rust
pub fn handle_marker_interaction(
    state: &mut OverlayState,
    response: &egui::Response,
    cursor_pos: Option<egui_plot::PlotPoint>,
) {
    let Some(pos) = cursor_pos else { return };

    if response.clicked() {
        state.markers.push(UserMarker {
            x: pos.x,
            label: format!("M{}", state.markers.len() + 1),
        });
    }
    if response.double_clicked() {
        // Find marker nearest to click — toggle editing state for rename/delete
        if let Some(idx) = state.markers.iter().position(|m| (m.x - pos.x).abs() < 0.5) {
            state.editing_marker = Some(idx);
        }
    }
}
```

- [ ] **Step 2: Draw user markers and mode change markers**

```rust
pub fn draw_markers(plot_ui: &mut egui_plot::PlotUi, user_markers: &[UserMarker], mode_changes: &[(f64, String)]) {
    let bounds = plot_ui.plot_bounds();

    // Mode change markers (dashed, auto-generated)
    for (x, label) in mode_changes {
        plot_ui.vline(
            egui_plot::VLine::new(*x)
                .color(egui::Color32::from_rgba_premultiplied(255, 200, 50, 120))
                .style(egui_plot::LineStyle::dashed_dense()),
        );
        // Label rendered via plot text
        plot_ui.text(
            egui_plot::Text::new(
                egui_plot::PlotPoint::new(*x, bounds.max()[1] * 0.95),
                egui::RichText::new(label).small().color(egui::Color32::from_rgb(255, 200, 50)),
            ),
        );
    }

    // User markers (solid)
    for marker in user_markers {
        plot_ui.vline(
            egui_plot::VLine::new(marker.x)
                .color(egui::Color32::from_rgb(255, 100, 100)),
        );
        plot_ui.text(
            egui_plot::Text::new(
                egui_plot::PlotPoint::new(marker.x, bounds.max()[1] * 0.9),
                egui::RichText::new(&marker.label).small().color(egui::Color32::from_rgb(255, 100, 100)),
            ),
        );
    }
}
```

- [ ] **Step 3: Generate mode change marker positions from data store**

In `mod.rs`, compute mode change positions from `data_store` for each visible file and pass to `draw_markers`.

- [ ] **Step 4: Add mode filter dropdown to viewer toolbar**

In `viewer_toolbar.rs`, add a dropdown that lists unique modes from all loaded files. Toggling a mode updates `LoadedFile::visible_modes`.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p readout-gui`

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/
git commit -m "feat(gui): add marker tool and mode change indicators to CSV Viewer"
```

---

## Chunk 5: CSV Viewer — Live Mode & Export

### Task 12: Live file detection and auto-follow

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`

- [ ] **Step 1: Verify live detection already works**

Live detection was already implemented inline in Task 6's `show()` method — the `OpenFile` and `AddFile` match arms compare the opened path against active recording paths from config. Verify this compiles and the `is_live` flag is correctly set. No new code needed.

- [ ] **Step 2: Implement auto-follow behavior**

In `render_chart()`, modify the Plot builder to include the latest data point when following:

```rust
let has_live = self.data_store.files().iter().any(|f| f.is_live);
let max_x = self.data_store.files().iter()
    .filter(|f| f.visible && f.is_live)
    .flat_map(|f| f.records.last())
    .map(|r| r.parsed_time.unwrap_or(0.0))
    .fold(0.0_f64, f64::max);

let mut plot = Plot::new("csv_viewer_plot")
    .allow_zoom(true)
    .allow_drag(true)
    .allow_scroll(true)
    .show_axes(true)
    .show_grid(true)
    .legend(egui_plot::Legend::default());

// When following, force the plot to include the latest X coordinate so it auto-scrolls
if self.following && has_live && max_x > 0.0 {
    plot = plot.include_x(max_x);
}
```

After `plot.show()`, detect user interaction that breaks follow:

```rust
let inner_response = response.response;
if inner_response.dragged() || (inner_response.hovered() && ui.input(|i| i.smooth_scroll_delta.y != 0.0)) {
    self.following = false;
}
```

- [ ] **Step 3: Verify ToggleFollow already wired**

`ToggleFollow` was already added to `ViewerAction` in Task 6 fix. The toolbar live indicator is already a clickable button (from viewer_toolbar.rs). The action is handled inline in `show()`. Verify this compiles correctly — no new code needed here, just confirmation.

- [ ] **Step 4: Build and verify**

Run: `cargo build -p readout-gui`

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/
git commit -m "feat(gui): add live file detection and auto-follow to CSV Viewer"
```

### Task 13: Export selection to CSV

**Files:**
- Modify: `readout-gui/src/widgets/csv_viewer/mod.rs`

- [ ] **Step 1: Implement export logic**

In `show()`, update the `ViewerAction::Export` match arm (currently a no-op):

```rust
ViewerAction::Export => {
    if let Some(save_path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_file_name("export.csv")
        .save_file()
    {
        export_to_csv(&save_path, &self.data_store, &self.overlay);
    }
}
```

Add standalone function (not a method, to avoid borrow issues):

```rust
fn export_to_csv(path: &std::path::Path, data_store: &CsvDataStore, overlay: &overlay::OverlayState) {
    use std::io::Write;
    let Ok(mut file) = std::fs::File::create(path) else { return };
    let _ = writeln!(file, "timestamp,device,value,unit,mode,is_overload,is_open,is_short");

    for loaded in data_store.files() {
        if !loaded.visible {
            continue;
        }
        let records = if let Some((x_min, x_max)) = overlay.selection {
            // Export only selection range
            loaded.records.iter()
                .enumerate()
                .filter(|(i, _)| {
                    let x = *i as f64; // will be timestamp-based after timestamp parsing is refined
                    x >= x_min && x <= x_max
                })
                .map(|(_, r)| r)
                .collect::<Vec<_>>()
        } else {
            loaded.records.iter().collect()
        };

        for r in records {
            let _ = writeln!(file, "{},{},{},{},{},{},{},{}",
                r.timestamp, r.device, r.value, r.unit, r.mode,
                r.is_overload, r.is_open, r.is_short);
        }
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p readout-gui`

- [ ] **Step 3: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/
git commit -m "feat(gui): add CSV export with optional selection range"
```

### Task 14: Timestamp parsing refinement

**Files:**
- Modify: `crates/readout-core/src/csv_record.rs`
- Modify: `readout-gui/src/widgets/csv_viewer/data_store.rs`
- Modify: `crates/readout-core/tests/csv_record_test.rs`

- [ ] **Step 1: Add timestamp parsing to CsvRecord**

Check the actual timestamp format written by CsvLogger (look at `row.timestamp` format in csv_logger.rs). Parse into a duration-from-first-record for chart X axis.

Add to `CsvRecord`:

```rust
pub parsed_time: Option<f64>,  // seconds since epoch or since first record
```

Parse in `parse_row` using the actual format. Add test.

- [ ] **Step 2: Update data_store to use parsed timestamps**

Replace the placeholder `Duration::from_secs(i as u64)` in `query_points()` with actual parsed timestamps.

- [ ] **Step 3: Update overlay stats and export to use real time coordinates**

Update these functions to filter by `parsed_time` instead of record index:
- `overlay::compute_selection_stats()` — change `let x = *i as f64` to use `r.parsed_time.unwrap_or(0.0)`
- `export_to_csv()` — change selection range filter from index-based to time-based
- Auto-follow `max_x` calculation in `render_chart()` — use `parsed_time` from last record

- [ ] **Step 4: Run all tests**

Run: `cargo test -p readout-core csv_record`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/readout-core/src/csv_record.rs readout-gui/src/widgets/csv_viewer/data_store.rs crates/readout-core/tests/csv_record_test.rs
git commit -m "feat(core): parse real timestamps in CSV records for chart X axis"
```

### Task 15: Final integration and polish

**Files:**
- All csv_viewer files

- [ ] **Step 1: Keyboard shortcuts in viewer**

In `CsvViewerWindow::show()`, add keyboard handling:
- `M` → toggle Measure mode
- `S` → toggle Select mode
- `K` → toggle Marker mode
- `Esc` → cancel current action or clear selection
- `Cmd+O` → open file
- `Cmd+Shift+O` → add file

- [ ] **Step 2: Legend click-to-toggle and right-click-to-remove**

Update viewer toolbar file chips to be interactive:
- Left click: toggle `file.visible`
- Right click: context menu with "Remove" option

- [ ] **Step 3: Full build and manual test**

Run: `cargo build -p readout-gui`
Run the app. Test:
- Open CSV Viewer from toolbar
- Open a CSV file
- Zoom/pan
- Add second file (overlay)
- Switch interaction modes
- Place markers
- Make measurements
- Select range, check stats
- Export

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/widgets/csv_viewer/ readout-gui/Cargo.toml
git commit -m "feat(gui): CSV Viewer keyboard shortcuts, legend interaction, and polish"
```

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p readout-gui -p readout-core -p readout-persistence -- -D warnings`
Fix any warnings.

- [ ] **Step 6: Final commit if clippy fixes needed**

```bash
git commit -m "fix: resolve clippy warnings"
```
