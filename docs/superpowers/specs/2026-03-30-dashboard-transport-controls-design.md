# Dashboard Transport Controls — Design Spec

## Overview

The dashboard title bar currently exposes a trash-style `Clear Charts` action, while
the existing `Pause/Resume` behavior is only reachable from the right-click context
menu. The underlying behavior is also split in a way that is not obvious from the UI:

- dashboard pause freezes dashboard values and dashboard charts
- runtime acquisition keeps running
- CSV logging keeps running
- `Clear Charts` only clears in-memory chart history

This change introduces explicit dashboard transport controls in the main title bar and
aligns the context menu with the same model:

- `Play/Pause` controls dashboard freeze/resume
- `Stop` clears dashboard chart history and leaves the dashboard paused

The transport controls are dashboard-only. They do not start or stop runtime capture,
and they do not start or stop CSV logging.

## Goals

- Make dashboard freeze/resume discoverable without requiring the context menu
- Replace the ambiguous trash-only interaction with clearer transport semantics
- Keep dashboard controls visually and behaviorally separate from device-level CSV logging
- Preserve current runtime and CSV logging behavior during pause
- Make `Stop` explicitly mean `clear + pause` for dashboard charts

## Out of Scope

- Changing runtime start/stop lifecycle
- Changing device-level `Start/Stop CSV logging` controls
- Changing `Graph Viewer` behavior
- Clearing or truncating CSV log files
- Pausing persistence writers, OBS output, or runtime event generation

## Current Behavior

### Pause

The dashboard already has a `paused` flag in `DashboardState`.

When `paused == true`:

- dashboard chart pipelines stop receiving new samples
- dashboard latest measurement values stop updating
- runtime events still continue to arrive in the app process
- persistence writers still run before GUI state handling, so CSV logging continues

This is the correct underlying behavior and should be preserved.

### Clear Charts

`Clear Charts` currently clears the in-memory dashboard chart pipelines only.

It does not:

- pause the dashboard
- change any CSV logging setting
- stop runtime acquisition
- clear any CSV file on disk

This behavior is technically safe, but the title bar affordance is not very clear and
it does not match the requested dashboard transport model.

## Proposed Behavior

### Dashboard Transport Model

The dashboard gets two explicit transport controls:

- `Play/Pause`
- `Stop`

These controls affect dashboard presentation state only.

### Play/Pause

`Play/Pause` maps directly to the existing dashboard `paused` state.

When running:

- button shows `Pause`
- clicking it sets `paused = true`

When paused:

- button shows `Play`
- clicking it sets `paused = false`

Behavior while paused remains unchanged:

- dashboard values remain frozen
- dashboard charts remain frozen
- runtime continues
- CSV logging continues
- OBS output continues

Resuming with `Play` does not rebuild old history. The dashboard simply starts accepting
new incoming samples again from that moment forward.

### Stop

`Stop` means:

- clear dashboard chart history
- set dashboard `paused = true`

This is intentionally stronger than the current `Clear Charts` behavior, because the
requested UX is `stop as clear + pause`.

`Stop` does not:

- change any CSV logging enable flag
- change runtime connection state
- stop background persistence writers
- remove or truncate CSV files

After `Stop`, pressing `Play` resumes dashboard updates and the charts refill only with
new samples arriving after resume.

## UI Design

### Title Bar

The main title bar should expose both transport controls directly.

Recommended arrangement on the right-side control cluster:

- range selector
- `Play/Pause`
- `Stop`
- settings
- `Graph Viewer`
- pin

`Play/Pause` should sit next to `Stop` so the transport pair reads as one control group.
The exact ordering can follow the existing toolbar spacing, but the two transport
controls should remain adjacent.

### Context Menu

The right-click context menu should mirror the same transport model:

- `Pause` or `Play`
- `Stop`

This keeps the context menu consistent with the title bar instead of exposing a
different action vocabulary.

### Copy and Tooltips

The UI should be explicit that these are dashboard controls, not logging controls.

Recommended copy:

- `Pause dashboard updates`
- `Resume dashboard updates`
- `Stop dashboard charts (clear and pause)`

The label shown directly in the title bar can stay icon-first to preserve compactness,
but hover text should spell out the behavior.

### Icons

Recommended icon mapping:

- `Pause` uses `PAUSE`
- `Play` uses `PLAY`
- `Stop` uses `STOP` or, if needed by style constraints, a square stop icon variant

The title bar should stop using the trash icon for this dashboard action.

## Internal Naming Boundary

The internal action name may remain `ClearCharts`.

Reason:

- the existing action routing already means `clear/reset chart history`
- the new UX meaning of `Stop` is implemented as `clear + pause`
- renaming every internal symbol to `Stop` is not required to deliver the behavior

UI naming and internal action naming can intentionally differ here as long as the
behavioral contract is clear and well-tested.

## Data Flow

### Toolbar State

The title bar state needs access to the current dashboard paused flag so it can render
the correct `Play` or `Pause` affordance.

Add to toolbar state:

- `paused: bool`

### Action Handling

Expected action handling:

- `TogglePause`
  - flips `state.paused`
  - changes nothing else

- `ClearCharts`
  - clears all dashboard chart pipelines
  - sets `state.paused = true`
  - changes nothing in runtime or persistence configuration

This keeps the action surface small while still delivering the requested UX.

## Behavioral Contract

The following cases must hold:

1. `Pause` freezes dashboard values and charts, but CSV logging continues.
2. `Play` resumes dashboard updates without touching CSV logging config.
3. `Stop` clears dashboard charts and leaves the dashboard paused.
4. `Stop` does not truncate or modify CSV files on disk.
5. Device-level `Start/Stop CSV logging` buttons remain the only logging transport controls.

## Testing

Add or update tests for:

- toolbar state and labels expose `Play/Pause` semantics in the title bar
- context menu uses `Play/Pause` plus `Stop`
- `TogglePause` only changes dashboard paused state
- `ClearCharts` clears all dashboard chart pipelines
- `ClearCharts` also forces `state.paused = true`
- `ClearCharts` leaves CSV logging configuration unchanged

Regression intent:

- prevent future confusion where a dashboard control accidentally mutates runtime
  logging settings
- prevent `Stop` from drifting into a runtime-stop feature

## Risks

- The label `Stop` can still be misread as `stop logging` if hover text is vague.
- Reusing internal `ClearCharts` naming is safe, but implementers must not forget the
  new `pause = true` side effect.
- Compact title-bar spacing may need a small adjustment once a second transport control
  is visible.

## Acceptance Criteria

- The title bar visibly exposes `Play/Pause` and `Stop`
- The context menu exposes the same control model
- `Pause` freezes dashboard values/charts only
- `Stop` performs `clear + pause`
- CSV logging continues uninterrupted during both `Pause` and `Stop`
