# Graph Viewer Render Cache — Design Spec

## Overview

`Graph Viewer` is now viewport-aware in the sense that it only queries and downsamples
the currently visible X-range. That fixed the correctness issue where live tails could
draw false diagonals from whole-history downsampling. It did not fully solve rendering
cost.

The current render path still rebuilds per-source render data on every frame:

- re-filters visible modes
- re-collects raw visible `DataPoint`s
- re-runs viewport downsampling
- re-allocates the final `Vec<[f64; 2]>` passed to `egui_plot`

That means live follow, small pan gestures, and repeated redraws continue to do nearly
the same work over and over even when neither the source data nor the effective viewport
has materially changed.

This change adds a Graph Viewer-local render cache so the window reuses prepared render
series whenever the source revision, visible modes, viewport span, and point budget are
still compatible. The goal is faster and smoother plotting without changing the data
contract introduced by viewport-aware rendering.

## Goals

- Improve `Graph Viewer` live rendering smoothness without reducing data fidelity
- Reduce repeated per-frame allocations and per-frame resampling work
- Keep `pan`, `zoom`, and `follow` responsive across long histories
- Reuse cached render series when only a small viewport movement occurs
- Keep cache ownership inside `Graph Viewer`, not in shared core/downsampling code
- Add tests for cache invalidation and reuse behavior

## Out of Scope

- Changing the dashboard mini-chart rendering path
- Replacing the existing viewport-aware downsampling algorithm
- Adding a full multi-resolution LOD index in this iteration
- Smoothing or interpolating the curve beyond the current raw/downsampled point model
- Changing CSV parsing, source attachment semantics, or live tail source types

## Current Bottleneck

The main cost is no longer repaint cadence. The main cost is repeated rebuild work inside
the Graph Viewer render loop.

Today the viewer:

1. reads visible bounds from the plot
2. computes a visible point budget from plot width
3. asks the data store for visible points for each source
4. rebuilds a fresh `Vec<DataPoint>`
5. downsamples that vector if needed
6. converts the result into a fresh `Vec<[f64; 2]>`

This work repeats on every frame even when:

- the source data did not change
- visible mode filters did not change
- the plot width is effectively the same
- the viewport only moved slightly within the same zoom level

The result is unnecessary CPU work and allocation churn in the exact paths used most
often during live observation.

## Architecture

### Cache Ownership Boundary

The render cache belongs to `Graph Viewer`, not to `CsvDataStore`.

Responsibilities:

- `CsvDataStore`
  - remains the source of truth for samples, mode visibility, and source metadata
  - exposes raw queryable source data
  - tracks whether a source changed in a way that affects rendering

- `GraphViewerWindow`
  - owns render caches
  - decides when cached series can be reused
  - decides when a viewport change requires a rebuild
  - supplies cached or rebuilt series to `egui_plot`

This keeps caching close to the UI behavior it optimizes and avoids turning the data
store into a rendering subsystem.

### Source Revision Tracking

Each `ViewerSource` gets a lightweight `render_revision: u64`.

The revision increments whenever a change can affect rendering, including:

- loading or replacing a CSV source
- appending live tail rows
- pushing runtime live samples
- changing visible mode filters for that source
- any other metadata change that alters which plotted points should exist

The cache never compares full vectors to detect changes. It only compares stored revision
numbers against the current source revision.

### Two-Layer Cache Model

Each visible source gets two related cache layers.

#### 1. Filtered Source Cache

This cache stores already prepared raw renderable points for the current source revision
and visible mode filter state.

It removes repeated work currently done each frame:

- `visible_modes.contains(...)`
- dropping `None` values
- converting `ViewerSample` into `DataPoint`

This cache is invalidated whenever the source revision changes.

#### 2. Viewport Series Cache

This cache stores the final plot-ready `Vec<[f64; 2]>` for a compatible viewport.

Stored inputs include:

- source id
- source revision
- cached X-range
- cached X-span
- cached point budget
- cached overscan range
- final series for `egui_plot`

The viewport cache is built from the filtered source cache, not from raw source samples.

## Viewport Reuse Strategy

### Overscan Window

The viewport cache should not be keyed to the exact visible range only. That would still
invalidate on every tiny pan movement.

Instead, each cached viewport series is built with a small overscan margin around the
visible range. Small movements inside that overscan region reuse the existing series.

Conceptually:

- visible range = what the plot is currently showing
- cached overscan range = visible range plus extra margin on both sides

Cache reuse is allowed when:

- current viewport is fully contained in cached overscan range
- the current point budget is unchanged
- the current source revision is unchanged
- the zoom span is still compatible with the cached series

The overscan margin does not change data semantics. It only reduces needless rebuilds
for small panning motions.

### Zoom Compatibility

The cache should also remember the viewport span used to generate the series.

If the user zooms enough that the current span materially differs from the cached span,
the viewport cache must rebuild even if the viewport is still numerically inside the old
overscan window. This prevents reusing a series generated for the wrong density.

The cache may tolerate tiny floating-point or one-pixel-level differences, but not large
span changes.

### Point Budget Compatibility

The current point budget comes from plot width. If that budget changes, the viewport cache
must rebuild.

This covers:

- window resize
- DPI/scale effects that change effective plot width
- layout changes that alter available chart width

## Follow And Live Tail Behavior

### Follow Reuse

In follow mode, if the viewport span is unchanged and a live source only appended new
samples, the cache should avoid rebuilding the entire series from scratch when possible.

Preferred behavior:

- reuse the existing cached viewport metadata
- append newly visible tail points to the series
- drop old left-edge points that moved outside the cached overscan window
- rebuild only when append/drop logic is no longer sufficient

This keeps live follow cheap while preserving the current viewport-aware downsampling
contract.

### Safe Fallback

The initial implementation does not need to perfectly incrementalize every case.

If incremental follow reuse becomes ambiguous, the renderer may rebuild the viewport
series from the filtered source cache. The key requirement is that the common case
should reuse cached results rather than always recomputing.

Correctness remains more important than aggressive reuse.

## Data Flow

Expected render flow:

1. Graph Viewer computes visible bounds and point budget
2. For each source:
   - skip hidden sources
   - check source revision
   - reuse or rebuild filtered source cache
   - attempt viewport cache reuse
   - if reuse fails, rebuild viewport series from the filtered source cache
3. Pass cached or rebuilt `Vec<[f64; 2]>` to `egui_plot`

This keeps the existing visible-range-first model while removing redundant work between
frames.

## Invalidation Rules

The cache must invalidate when any of the following changes:

- source revision
- source visibility
- visible mode filter for the source
- viewport span beyond tolerance
- point budget
- plot width
- source removal
- follow state in a way that changes viewport behavior

Invalidation must be per-source where possible. One source appending live data must not
force every other source to rebuild.

## Error Handling

Render caching is a performance feature, not a correctness dependency.

Requirements:

- cache misses are normal and must fall back to rebuilding
- stale or incompatible cache entries must be dropped silently and rebuilt
- cache logic must never reintroduce whole-history downsampling
- if cache state becomes inconsistent, the viewer should rebuild from current source data

The fallback path must always preserve the existing viewport-aware rendering contract.

## Testing

Add targeted tests for the new caching model:

- source revision increases when plotted source data changes
- source revision increases when mode visibility changes
- cache reuse is allowed when revision, overscan range, and point budget are unchanged
- cache invalidates when revision changes
- cache invalidates when viewport span changes materially
- cache invalidates when point budget changes
- repeated renders with unchanged inputs reuse cached output instead of rebuilding

Existing viewport-aware rendering tests should continue to pass unchanged. The cache must
optimize the current behavior, not redefine it.

## Verification

Before implementation is claimed complete:

- `cargo test -p readout-gui`
- `cargo clippy -p readout-gui -- -D warnings`

If implementation pressure pushes logic into shared crates, expand verification to the
affected crates as needed.
