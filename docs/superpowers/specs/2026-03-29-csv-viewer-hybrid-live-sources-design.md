# CSV Viewer Hybrid Live Sources — Design Spec

## Overview

CSV Viewer should stop treating "live" as a side effect of opening a configured CSV file.
It should support three explicit source types in one overlay chart:

1. Historical CSV files opened via file picker
2. Live CSV tail sources attached from configured log paths
3. Direct runtime live sources attached from the running app

The goal is to keep file-based inspection, keep and improve live CSV tailing, and add a smooth runtime-backed live path that behaves more like the main dashboard charts while keeping its own longer viewer-local history.

## Goals

- Keep `Open` and `Add` for historical CSV analysis
- Keep live CSV tailing, but make it explicit and noticeably smoother
- Add `Attach Live` actions that stream directly from runtime measurements, independent of CSV logging
- Allow CSV, live-tail, and runtime sources to overlay in the same plot
- Fix current viewer robustness issues around follow, hover, tooltips, and timestamp handling
- Keep the current single-plot layout for v1

## Out of Scope

- Split view / stacked plots with shared X axis
- Global background history collected before the viewer attaches
- Replacing the main dashboard charts
- Persisting runtime history across viewer close/reopen
- Exposing extra USB-C secondary metrics in the viewer; v1 keeps parity with the current CSV representation and shows the primary plotted value only
- Making runtime attach implicitly enable CSV logging

## UX

### Toolbar

The viewer toolbar becomes:

`[Open] [Add] [Attach Live ▾] [Tail CSV ▾] | [Fit] [Measure] [Select] [Marker] [Modes ▾] [Export]`

Menu contents:

- `Attach Live ▾`
  - `Multimeter`
  - `USB-C`
- `Tail CSV ▾`
  - `Multimeter CSV`
  - `USB-C CSV`

Right side of the toolbar keeps source chips and the live follow indicator.

### Source Semantics

- `Open` replaces the current source set with one historical CSV source
- `Add` appends another historical CSV source
- `Attach Live` creates a runtime-backed source and starts collecting viewer-local history from that moment forward
- `Tail CSV` attaches a file-backed live source using the configured CSV log path for that device

Runtime attach is independent from CSV logging. If logging is enabled elsewhere in the app, the same device may appear both as a runtime live source and as a tailed CSV source.

### Source Chips

Each attached source gets a chip:

- Historical CSV: filename
- Live CSV tail: `MM Tail` / `USB-C Tail`
- Runtime live: `MM Live` / `USB-C Live`

Chip interactions:

- click: hide/show
- context menu: remove/detach

Duplicate attachment of the exact same source is ignored instead of creating another chip.

## Architecture

### High-Level Model

Replace the current file-only `CsvDataStore` model with a viewer source registry that can host different source kinds behind one plotting interface.

Core concepts:

- `ViewerSourceId`
- `ViewerSourceKind`
  - `CsvFile { path }`
  - `LiveCsvTail { device, path }`
  - `RuntimeDevice { device }`
- `ViewerSeries`
  - source identity and label
  - visibility and color
  - normalized samples
  - mode metadata and auto markers
  - source-local status and error state

The render layer should consume normalized series data and should not care whether a point came from a file, a tailed log, or runtime events.

### Normalized Sample Model

The current viewer assumes every X value is an epoch timestamp. That breaks on CSV rows that have no parsable RFC3339 timestamp and makes runtime integration awkward because runtime events use monotonic `Instant`.

The viewer should normalize every source into samples with an explicit X-domain:

- `WallClock(DateTime<Utc>)`
- `SequenceIndex(usize)`

Each plotted sample keeps:

- X domain value
- numeric value
- unit
- mode
- overload/open/short flags
- display timestamp text

Behavior:

- Parsed readOutRS CSV rows use `WallClock`
- Runtime live uses `WallClock`
- CSV rows without parsable RFC3339 timestamps use `SequenceIndex`
- Sources with incompatible X domains are not mixed silently; attaching an incompatible source shows an explicit viewer error instead of rendering fake `1970` timestamps

This keeps generic CSV support without corrupting the axis model.

### Runtime Live Ingestion

`ReadOutApp` already drains `RuntimeEvent::Measurement` and feeds `DashboardState`.
The viewer should subscribe to the same event flow in-process.

Design:

- `ReadOutApp::update` continues to call `self.state.handle_event(event)`
- in the same drain loop it also forwards measurement events into the CSV viewer, if the viewer has runtime sources attached
- forwarding is best-effort and UI-local; it does not feed back into runtime or persistence

Runtime viewer history is local to the viewer window:

- no buffer exists before `Attach Live`
- first attach creates the runtime source buffer
- removing the source drops that history
- closing the viewer drops all runtime source history

### Runtime Timebase

Runtime events carry monotonic `Instant`, not serializable wall-clock time.
To render runtime series on the same wall-clock axis as CSV logs, the viewer should anchor each runtime source on first sample:

- capture wall-clock `Utc::now()` when the first attached sample is received
- capture that sample's monotonic timestamp
- derive later wall-clock timestamps by applying monotonic deltas to the anchor

This keeps runtime live aligned with CSV timestamps closely enough for overlay and export, without changing the runtime event type.

### Live CSV Tailing

Live CSV tail remains file-backed, but it becomes explicit and more responsive.

Behavior:

- `Tail CSV` uses configured `*_csv_log_file_path`
- the action is available when a path is configured, regardless of whether logging is currently enabled
- if the file is missing or unreadable, the source remains attached with an explicit source error
- if the file later appears or grows, the source starts updating without re-attaching

Implementation expectations:

- keep incremental append parsing and incomplete trailing line handling
- reduce poll cadence from `1 s` to a much shorter interval suitable for smooth updates
- keep truncation/rotation detection by comparing file size and resetting read position when needed
- keep local-only assumptions for now; no network filesystem guarantees

## Plot Behavior

### Follow

Follow should be a real viewport behavior, not only a boolean label.

Rules:

- enabling follow immediately snaps the plot to the live right edge
- new live data from either runtime or tailed CSV keeps the plot pinned to the right edge while follow is on
- manual pan/zoom disables follow
- if no live-capable source is attached, the follow control is hidden

### Hover and Tooltip

There should be one source of truth for hover readout.

Changes:

- remove the current split between `egui_plot` label formatter tooltip and overlay text tooltip
- hover resolution should select the actual nearest visible plotted sample, not only the globally nearest X value
- tooltip content should identify the concrete source, timestamp/index, value, unit, and mode

The info bar should reuse the same resolved hover sample instead of recomputing from different heuristics.

### Mode Markers and Filters

Existing mode markers and mode filters stay, but they move from file-specific logic to series-specific logic.

Rules:

- each source computes its own mode change markers
- mode filters still apply across all visible series
- runtime live sources update their marker set incrementally when mode changes appear in the live stream

## Export

Export should operate on visible plotted data regardless of source kind.

Rules:

- historical CSV and tailed CSV export in the current readOutRS CSV format
- runtime live export uses the viewer-local buffered history and writes the same CSV shape
- runtime export timestamps use the viewer's derived wall-clock timestamps
- if a selection range is active, export respects that range

## Error Handling

The viewer should surface source-local failures explicitly instead of silently degrading.

Cases to handle:

- runtime attach requested for a device that is not connected or has no measurements yet
- `Tail CSV` requested with no configured path
- configured CSV path is unreadable or missing
- incompatible X-domain source attach
- export failure

Presentation:

- keep the existing top-level error banner for action failures
- add per-source status/error text in the chip tooltip or source metadata area so one broken source does not make the whole viewer feel undefined

## Testing

### Unit Tests

- source registry add/remove/dedup behavior
- runtime sample normalization and wall-clock anchoring
- incompatible X-domain rejection
- live CSV incremental append, truncation, and incomplete trailing row handling
- follow state transitions
- nearest-sample hover resolution across multiple visible series
- runtime export and CSV export range filtering

### Integration-Level GUI Tests

- attach runtime source, receive measurements, and render non-empty series
- attach live CSV source and observe incremental growth
- manual pan disables follow, follow re-enable snaps to live edge
- source chips hide/show/remove the intended series

## Migration Notes

The current `CsvDataStore { files: Vec<LoadedFile> }` shape is too narrow for this feature set.
Implementation should move toward:

- source registry instead of file list
- source-kind-specific update paths
- series-oriented plotting and hover hit testing

This is the structural change that also prepares the viewer for future split-view work, without putting split view itself into this scope.
