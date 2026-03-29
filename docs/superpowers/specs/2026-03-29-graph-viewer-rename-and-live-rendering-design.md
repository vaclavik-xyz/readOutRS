# Graph Viewer Rename And Live Rendering — Design Spec

## Overview

The current `csv_viewer` feature has outgrown its name and its rendering strategy.
It no longer works only with static CSV files: it can display historical CSV files,
live CSV tails, and runtime live sources. At the same time, the current rendering
path downsamples entire source histories to a fixed global limit before plotting.
That produces misleading line segments in live mode when the visible window is only
showing the newest portion of a much larger dataset.

This change renames the feature to `Graph Viewer` at the feature/module level and
reworks live plotting so the viewer stays responsive without drawing a shape that is
not present in the underlying data.

## Goals

- Rename the feature from `CSV Viewer` to `Graph Viewer` in UI and internal feature-level identifiers
- Give the Graph Viewer opener a distinct icon from the device chart visibility controls
- Keep CSV-specific actions and source kinds explicitly named as CSV where that is the real source type
- Fix `Tail CSV` live rendering so the plotted line does not invent diagonal or broken segments
- Keep the viewer responsive with long histories and active live sources
- Add regression coverage for the renamed feature and the new live rendering contract

## Out of Scope

- Replacing CSV terminology inside CSV-specific parser, import, export, or tail actions
- Adding new source types beyond historical CSV, tailed CSV, and runtime live
- Reworking the main dashboard charts
- Building a full multi-resolution cache system in this iteration
- Redesigning the overall Graph Viewer layout beyond the naming/icon updates

## UX

### Naming

The feature is presented as `Graph Viewer`.

Applies to:

- toolbar hover text and labels
- window title
- feature-level action names
- feature-level internal identifiers such as viewport IDs and plot IDs where they currently say `csv_viewer`

Does not apply to CSV-specific operations:

- `Tail CSV`
- CSV import/open behavior
- CSV export format
- CSV-specific data model or parser names

### Iconography

The Graph Viewer open action should use a different icon than the small device chart
visibility control. The distinction should be semantic:

- device chart icon: show or hide the inline chart for one device
- Graph Viewer icon: open the separate analysis window

The new icon only needs to be clearly different and already available in the icon set;
it does not need a broader toolbar redesign.

## Architecture

### Feature Rename Boundary

Rename the feature-level module and types from `csv_viewer` to `graph_viewer`.

Expected rename surface:

- widget module path
- window type names such as `CsvViewerWindow`
- toolbar action names such as `OpenCsvViewer`
- viewport and plot identifiers
- tests whose names are feature-level rather than CSV-source-level

Keep CSV-specific names where they describe real CSV behavior:

- `attach_live_csv`
- `Tail CSV`
- `CsvRecord`
- CSV export/import helpers

This keeps the feature naming accurate without hiding which sources are actually CSV-backed.

### Rendering Root Cause

Today the viewer collects points via a fixed global call like `query_points(source_id, 2000)`
and then draws a line from those returned points. That means downsampling happens over the
entire source history instead of over the currently visible X-range. In live mode, the newest
visible window may then contain only a few representative points chosen from a much larger
history, and the line renderer connects them into shapes that were never present in the source data.

The problem is therefore not only live-tail ingestion. The main issue is global downsampling
without viewport awareness.

### New Plot Query Model

The render path should become viewport-aware.

The data store should support querying points for a specific visible X-range, not just a fixed
point budget across the full source.

Conceptually:

- renderer determines current visible X bounds
- renderer asks the data store for points in that visible window
- if the visible window contains a manageable number of points, render raw points
- if the visible window contains too many points, downsample only within that visible window

The render path should derive the point budget from visible plot width rather than using a fixed
magic number for every situation.

### Downsampling Contract

Downsampling is allowed for performance, but it must not invent a curve shape that is not present
in the data.

Required properties:

- output points remain ordered by `x`
- first visible point is preserved
- last visible point is preserved
- representative points from each bucket are emitted in time order
- if a bucket contributes multiple points, they must still be emitted in monotonic `x` order
- when the visible window has few enough points, raw points are used instead of downsampling

The practical target is: the viewer may simplify history, but it must never create a false diagonal,
false reversal, or other line segment that implies missing intermediate data which was not actually
selected from the visible range.

### Live Rendering Strategy

`Tail CSV` and runtime live sources should use the same viewport-aware rendering rules.

Behavior:

- newest visible live segment must preserve its local shape
- long historical tails may still be simplified for performance
- simplification must only happen inside the currently visible range
- manual zoom into a smaller window should naturally reveal more raw detail

If viewport bounds or plot width cannot be determined reliably for a frame, the renderer should
fall back to a conservative safe mode that prefers correctness over aggressive simplification.
That means rendering more raw points from the active window rather than using a misleading global
downsample.

## Error Handling

The existing source-local error behavior should remain in place.

For rendering-specific fallback:

- failure to compute a viewport-aware downsample should not break plotting
- the fallback path should render a safe subset or raw visible points
- the viewer should not silently return to global whole-history downsampling if that would reintroduce false shapes

## Testing

### Rename Coverage

Add or update tests to cover the feature-level rename where behavior already exists in testable form:

- Graph Viewer action naming
- toolbar action wiring
- window/type-level feature references where currently covered by unit tests

### Rendering Regression Coverage

Add regression tests for viewport-aware live rendering:

- a long source history plus a small cluster of newest live points should preserve the local shape of the newest segment
- downsample output must remain monotonic in `x`
- first and last visible points must survive viewport downsampling
- when the visible range is small enough, raw points should be returned unchanged

Tests should target the new query/downsampling contract directly where possible, instead of only
asserting end-to-end UI state.

## Verification

Before implementation is claimed complete:

- `cargo test -p readout-gui`
- `cargo clippy -p readout-gui -- -D warnings`

If the implementation touches shared downsampling code in `readout-core`, extend verification to
the affected crates as needed.
