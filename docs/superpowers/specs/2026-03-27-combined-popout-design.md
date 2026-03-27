# Combined Popout Window

Replaces the two separate popout windows (Multimeter, USB-C) with a single combined popout that serves as a compact monitoring overlay for corner-of-screen use while working on schematics or other tasks.

## Layout

Single always-on-top window, vertical stack. Default size ~320x500px, min 280x300.

Structure top-to-bottom:

1. **Compact toolbar** (1-2 rows)
2. **Multimeter section** (optional)
3. **USB-C section** (optional)

Device visibility is configurable — user can show one or both devices. When only one is visible, the other section disappears and the window shrinks. At least one device must always be visible — toggling off the last active device is a no-op.

## Compact Toolbar

Single horizontal row, small font (~10-11px). If content overflows, split into two rows.

Row contents (left to right):
- Device visibility: selectable labels **MM** | **USB-C** (one or both active)
- Separator
- **⏸/▶** Pause/Resume button
- Separator
- Beep toggles: **🔊 PC** | **🔔 Meter** (selectable labels)
- Separator
- USB-C metric: **V** / **A** / **W** / **mWh** (selectable labels, controls USB-C chart)
- Time range: **2m** / **5m** / **10m** / **30m** / **1h** (selectable labels, shared for both charts)

## Device Section

Each device section (MM / USB-C) has identical structure:

### Measurement Display
- Device name (11px, secondary color) left-aligned + connection LED indicator right-aligned
- Primary value: monospace, ~28-30px
- Mode string: 11px, secondary color
- **USB-C only**: secondary row `0.000 A | 0.000 W` (14px monospace)
- **USB-C only**: energy row `123.4 mWh` + **↺ Reset** button

### Mini Chart
- Height: ~80px, full width
- Single line: MM blue (`rgb(60,170,250)`), USB-C orange (`rgb(255,160,60)`)
- Uses shared time range from toolbar
- USB-C chart shows selected metric from toolbar
- No legends, no interaction — visual trend overview only
- Thin separator between sections

### Alarm States
Same alarm badge and background tint as main window, smaller font.

## Data Flow

### Into Popout (from app.rs)
- Both `DeviceMeasurement` (cloned)
- Connection state and alarm state for both devices
- Chart data: pre-queried `Vec<[f64;2]>` from chart pipelines for MM + selected USB-C metric (bypasses non-Clone `ChartPipeline`)
- Current state: pause, beep flags, selected USB-C metric, time range, device visibility

### Actions from Popout (return enum)
`PopoutAction` enum (same pattern as `HeaderAction`):
- `TogglePause`
- `TogglePcBeep`
- `ToggleMeterBeep`
- `ResetEnergy` — sends `Command::ResetEnergy { device: DeviceId::UsbC }`
- `SetUsbcMetric(UsbCMetric)`
- `SetTimeRange(usize)` — index into RANGE_OPTIONS
- `ToggleDeviceVisibility(DeviceId)`
- `Close`

State changes (metric, range, device visibility) are shared with main window — changing metric in popout reflects in the main chart and vice versa.

## Codebase Changes

### Modified Files
- **`popout.rs`** — complete rewrite: expanded `PopoutState` (device visibility fields), `PopoutAction` enum, `show_combined_popout()` function
- **`app.rs`** — in `update()`: pre-query chart data, pass expanded parameters to popout, handle `PopoutAction` (analogous to `HeaderAction` handling)
- **`header.rs`** — simplify: replace two "⬒ MM" / "⬒ USB-C" buttons with single "⬒ Popout" toggle

### Removed
- Old `PopoutDisplayMode`, `PopoutLayoutProfile`, `PopoutWindowFrame` from config — unused and superseded by this design

### Unchanged
- `theme.rs`, `device_card.rs`, `chart.rs`, `status_strip.rs`, `log_panel.rs` — main window unchanged
- Runtime, core, persistence crates — no changes (energy reset command already exists)

### Config Persistence
- Popout open/closed state and device visibility saved to `AppConfiguration` so popout restores state on restart
