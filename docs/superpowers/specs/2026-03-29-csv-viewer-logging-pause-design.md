# CSV Viewer & Logging Pause — Design Spec

## Overview

Two features for readOutRS GUI:
1. **CSV Viewer** — interactive chart window for analyzing CSV data (live + historical), with full analytics tooling
2. **Logging Pause** — clickable recording indicators that toggle CSV/OBS logging on/off per device

## Feature 1: CSV Viewer

### Architecture

Hybrid approach: `egui_plot` for chart rendering (axes, zoom, pan, grid, lines) + custom overlay layer for analytics (markers, measurements, selection stats).

No new crate. Implementation lives in `readout-gui/src/widgets/csv_viewer.rs` (potentially split into submodules) as a new viewport window.

### Components

**`CsvViewerWindow`** — main widget struct
- Opens as separate `egui::ViewportBuilder` window (like MeterControl/Settings)
- Manages: loaded files, chart state, tool state, markers, measurements
- Opened via toolbar button in main window

**`CsvDataStore`** — data layer
- Parses CSV files in readOutRS format: `timestamp,device,value,unit,mode,is_overload,is_open,is_short`
- Per-row parsed fields: all columns stored. Primary display uses `(timestamp, value)` for the chart line. `unit`, `mode`, `is_overload`, `is_open`, `is_short` are retained for tooltips, mode markers, and filtering.
- Holds data in memory as `Vec<CsvRecord>` per file (struct with all parsed fields)
- Downsamples for viewport rendering using min-max bucketing algorithm (same approach as `ChartPipeline::query`, extracted as standalone functions — not reusing the ring buffer struct itself)
- For live files: periodic re-read of new lines from file tail (1s poll interval). If file size shrinks (truncation/rotation), reset seek position to 0 and reload.

**Interaction layer** — custom overlay on top of `egui_plot`
- Uses `PlotResponse` for cursor position, bounds, hovered point
- State machine: Normal → Measuring → Selecting → MarkerPlacing

### Layout

```
┌─────────────────────────────────────────────────────┐
│ Toolbar: [Open][Add] | [Zoom Fit][Measure][Select]  │
│  [Marker] | [Export] | [Mode filter ▾]  🟢 Live     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Y-axis │          Chart area                       │
│         │   - egui_plot with Line per file           │
│         │   - Custom overlay (crosshair, markers,    │
│         │     measurements, selection highlight)     │
│         │   - Auto markers at mode/unit changes      │
│         │                                           │
│         └───────────── X-axis (time) ───────────────│
├─────────────────────────────────────────────────────┤
│ Info bar: 🖱️ 12.345V @ 10:32:15 | Min/Max/Avg/Std  │
│           | Δt: 1m 23s  Δv: 0.341V                  │
└─────────────────────────────────────────────────────┘
```

### Interaction Modes

| Mode | Activation | Behavior |
|------|-----------|----------|
| **Normal** | default | Hover = crosshair + tooltip (value + time + unit + mode). Scroll = zoom. Drag = pan. |
| **Measure** | toolbar / `M` | Click point A → drag → point B. Shows Δt, Δvalue, line between points. Multiple measurements allowed, Esc cancels last. |
| **Select** | toolbar / `S` | Drag rectangle → selected range. Info bar shows min/max/avg/stddev. Export selection to CSV. |
| **Marker** | toolbar / `K` | Click on chart = vertical marker with label. Markers survive zoom/pan. Double-click marker = rename/delete. |

### Info Bar

Always visible at bottom:
- **Left**: cursor position (value + timestamp)
- **Center**: selection statistics (when selection active) — min, max, avg, stddev
- **Right**: measurement deltas (when measurement active) — Δt, Δvalue

### Mode Change Handling

When the measurement mode or unit changes mid-file:
- **Auto markers** — vertical dashed line with label (e.g., "DCV → ACV") inserted automatically at the transition point. These are visual-only, not editable like user markers.
- **Mode filter** — dropdown in toolbar lists all modes found in loaded files. User can select which modes to display; unselected modes are hidden from the chart. Default: all modes visible.
- Mode/unit information shown in tooltip on hover.

### Overlay Mode (Multi-File)

- Multiple CSV files loaded into one chart
- Each file rendered as separate `egui_plot::Line` with distinct color
- Legend in toolbar: colored chips with filename, click = toggle visibility, right-click = remove
- Analytics tools operate on visible series
- Future: split view (separate panels per file with shared time axis) — out of scope for v1

### Live Mode

- **Detection**: viewer compares opened file path with active `*_csv_log_file_path` in config where logging is enabled → match = live mode
- **Polling**: 1s timer reads new lines from file end (seek to last known position). If file size is smaller than last known position (truncation/rotation), reset to beginning and reload.
- **Auto-follow**: new data scrolls viewport right. Green indicator "Live · Following" in toolbar.
- **Detach**: any manual pan/zoom switches to "Live · Paused" view. "Follow" button (or double-click indicator) returns to auto-follow.
- **Post-hoc files**: no live indicator, no polling, static data.

### Entry Points

- Toolbar button in main window (opens empty viewer, user selects file via file picker)

## Feature 2: Logging Pause

### Architecture

Clicking the recording indicator in the device section header toggles the logging enabled flag. For CSV this is the existing `*_csv_logging_enabled` boolean. For OBS, new `multimeter_obs_enabled` and `usbc_obs_enabled` boolean fields are added to `AppConfiguration` (default: `true`, backward compatible — existing configs with a file path set will continue working).

### Behavior

- **Click CSV recording icon** → toggles `*_csv_logging_enabled` in config
- **Click OBS recording icon** → toggles new `*_obs_enabled` in config
- **Config save** → triggers runtime to stop/restart logger
- **Settings sync** → checkbox in settings panel reflects the same flag (bidirectional)
- **Append on re-enable** → `CsvLogger` already opens with `.append(true)` and writes header only to empty files. Re-enabling continues writing to the same file seamlessly.
- **OBS active condition** → OBS writer runs when `*_obs_enabled == true` AND `*_output_file` is non-empty

### Config Changes

New fields in `AppConfiguration`:
```rust
pub multimeter_obs_enabled: bool,  // default: true
pub usbc_obs_enabled: bool,        // default: true
```

Existing fields used as-is:
- `multimeter_csv_logging_enabled` / `usbc_csv_logging_enabled`

### Icon States

| State | Appearance | Condition |
|-------|-----------|-----------|
| Recording (CSV) | Red icon | `*_csv_logging_enabled == true` AND `*_csv_log_file_path` non-empty |
| Recording (OBS) | Green icon | `*_obs_enabled == true` AND `*_output_file` non-empty |
| Off | Grey / hidden | disabled OR no file path configured |

No intermediate "paused" state — logging is either on or off.

### Scope of Changes

- **`config.rs`** — add `multimeter_obs_enabled`, `usbc_obs_enabled` fields with `#[serde(default = "default_true")]`
- **`device_section.rs`** — recording indicators become clickable, return action on click
- **`app.rs`** — handles action, toggles config field, saves config, signals runtime
- **Runtime** — starts/stops logger based on config change (existing mechanism)

## Out of Scope (Future)

- Split view (multiple files in separate panels with shared time axis) — track as issue
- CSV Viewer in TUI
- Annotation persistence (save markers to file)
- Multi-device overlay in single CSV (current format is single-device per file)
- Keyboard shortcut conflicts audit (M, S, K keys in viewer — verify no egui conflicts during implementation)
