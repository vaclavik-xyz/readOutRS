# Compact Single-Window App

Replace the current dual-window architecture (main window + popout) with a single compact window that serves as the entire application. The popout becomes the app.

## Architecture

One `eframe` window. No secondary viewports, no popout concept. The current popout rendering becomes the main app render loop.

- Default size: ~320x500, min 280x250
- Starts with `always_on_top` enabled (toggleable via toolbar)
- First-run wizard displayed inline in this window
- Settings and logs accessible via overlay windows (`egui::Window`)

### Removed Code

- `widgets/device_card.rs` — main window device cards (replaced by compact device sections)
- `widgets/status_strip.rs` — bottom status bar (info moved to toolbar/overlays)
- `widgets/log_panel.rs` — bottom log panel (replaced by log overlay)
- `widgets/chart.rs` — large chart widget (replaced by mini charts in device sections). `RANGE_OPTIONS` and `ChartState` move into `app.rs` (or a small `chart_state.rs` module if cleaner).
- Popout viewport logic (`show_viewport_immediate`, `ViewportId`, popout-specific state)

### Preserved Code

- `theme.rs` — unchanged
- `widgets/settings.rs` — unchanged (already an `egui::Window` overlay)
- `widgets/first_run_wizard.rs` — unchanged (already renders as `egui::Window` overlay, works in any size)
- `audio.rs` — unchanged
- All runtime, core, persistence crates — unchanged

## Window Layout (top to bottom)

### Toolbar (2 rows, ~10-11px font)

**Row 1:**
- Device visibility: selectable labels **MM** | **USB-C** (at least one must stay active)
- Separator
- Pause/Resume button: **⏸** / **▶**
- Separator
- Beep toggles: **🔊 PC** | **🔔 M** (selectable labels)
- Separator
- Time range: **2m** / **5m** / **10m** / **30m** / **1h** (selectable labels, shared for both charts)

**Row 2:**
- **📋** Log button (opens log overlay)
- **⚙** Settings button (opens settings overlay)
- **📌** Always-on-top toggle (selectable label)

### Multimeter Section (optional)

- Title "Multimeter" (11px, secondary color) + connection LED (right-aligned)
- Primary value: ~28px monospace
- Mode string: 10px, secondary color
- Alarm badge (if active)
- Mini chart: 80px height, blue line, no axes

### USB-C Section (optional)

- Title "USB-C" (11px, secondary color) + connection LED (right-aligned)
- Primary value: ~28px monospace (voltage by default)
- Mode string: 10px, secondary color
- Secondary row: `0.000 A | 0.000 W` (14px monospace)
- Energy row: `123.4 mWh` + **↺ Reset** button
- **Metric selector row**: V / A / W / mWh selectable labels (controls which metric the mini chart shows)
- Alarm badge (if active)
- Mini chart: 80px height, orange line, no axes

### Separator between sections

Thin separator with 4px spacing when both devices are visible.

## Overlay Windows

### Settings (⚙)

Existing `egui::Window` overlay — no changes to content or behavior. Opens over the compact window content.

### Log (📋)

`egui::Window` overlay with:
- Scrollable list of log entries
- Monospace font, color-coded by level (error=red, warning=amber, info=default, debug=secondary)
- Close button (X) or click outside
- Max height constrained to not exceed window bounds

## Auto-Resize with Animation

When device visibility changes (toggle MM or USB-C on/off):
1. Calculate target height based on visible content (toolbar + visible sections)
2. Animate window height toward target using lerp (~200ms transition)
3. Width remains unchanged
4. Auto-resize only triggers on device visibility toggle — if the user manually resizes, that size is used as the base until the next visibility toggle

## Always-on-Top Toggle

- **📌** selectable label in toolbar row 2
- Selected state = always on top (default)
- Deselected = normal window behavior
- State persisted in config (`always_on_top: bool`, default `true`)
- Applied via `ViewportCommand::WindowLevel`

## Data Flow

Same as current popout, but simplified (no cross-viewport cloning needed):
- Chart pipelines queried directly in the render loop (no pre-cloning to Vec)
- Measurements, connections, alarms accessed directly from `DashboardState`
- Actions handled inline (no `PopoutAction` enum needed — direct state mutation)

Since everything runs in the main viewport, mutable access to `DashboardState` and `ChartPipeline` is direct — no ownership gymnastics.

## Config Changes

### New fields
- `always_on_top: bool` (default `true`)

### Removed fields
- `popout_open: bool` — no longer needed (always open)

### Retained fields
- `popout_show_mm: bool` → rename to `show_mm: bool`
- `popout_show_usbc: bool` → rename to `show_usbc: bool`
- All other config fields unchanged

## Keyboard Shortcuts

- **Cmd+P**: Pause/Resume
- **Cmd+L**: Toggle log overlay
- **Cmd+,**: Open settings
- **Cmd+1**: removed (no popout toggle needed)

## Status Information

Health metrics (measurement count, errors, reconnects) currently in status strip — shown as a compact line in the log overlay header. Not displayed permanently in the main UI to save space.
