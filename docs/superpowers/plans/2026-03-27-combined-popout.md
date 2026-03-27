# Combined Popout Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace two separate popout windows with a single combined popout containing both devices, mini charts, and full controls.

**Architecture:** Single egui viewport with vertical stack layout. Data flows from app.rs (pre-queried chart data + cloned state) into the popout via `PopoutInput` struct. Actions flow back via `PopoutAction` enum (same pattern as `HeaderAction`). Device visibility toggled directly on `PopoutState`.

**Tech Stack:** Rust, egui 0.33, egui_plot 0.34, eframe 0.33

---

### Task 1: Export RANGE_OPTIONS from chart.rs

**Files:**
- Modify: `readout-gui/src/widgets/chart.rs:8`

- [ ] **Step 1: Make RANGE_OPTIONS public**

In `readout-gui/src/widgets/chart.rs`, change line 8 from:

```rust
const RANGE_OPTIONS: &[(Duration, &str)] = &[
```

to:

```rust
pub const RANGE_OPTIONS: &[(Duration, &str)] = &[
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p readout-gui`
Expected: PASS (no consumers yet, just making it available)

- [ ] **Step 3: Commit**

```bash
git add readout-gui/src/widgets/chart.rs
git commit -m "refactor: export RANGE_OPTIONS from chart module"
```

---

### Task 2: Update config — remove unused popout types, add new fields

**Files:**
- Modify: `crates/readout-persistence/src/config.rs`
- Modify: `readout-gui/src/widgets/settings.rs:110`

- [ ] **Step 1: Remove unused type definitions from config.rs**

Remove the `PopoutDisplayMode` enum (lines 81-85):
```rust
case_insensitive_enum!(PopoutDisplayMode {
    Mini => "mini",
    Compact => "compact",
    Detailed => "detailed",
});
```

Remove the `PopoutWindowFrame` struct (lines 94-102):
```rust
// --- PopoutWindowFrame ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopoutWindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
```

Remove the `PopoutLayoutProfile` struct (lines 104-116):
```rust
// --- PopoutLayoutProfile ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopoutLayoutProfile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub multimeter_mode: PopoutDisplayMode,
    #[serde(default)]
    pub usbc_mode: PopoutDisplayMode,
    pub multimeter_frame: Option<PopoutWindowFrame>,
    pub usbc_frame: Option<PopoutWindowFrame>,
}
```

- [ ] **Step 2: Replace old popout fields in AppConfiguration struct**

In the `AppConfiguration` struct, replace the "Popout windows" section (lines 221-233):

```rust
    // Popout windows
    #[serde(default)]
    pub multimeter_popout_mode: PopoutDisplayMode,
    #[serde(default)]
    pub usbc_popout_mode: PopoutDisplayMode,
    pub multimeter_popout_frame: Option<PopoutWindowFrame>,
    pub usbc_popout_frame: Option<PopoutWindowFrame>,
    #[serde(default)]
    pub popout_alarm_emphasis_enabled: bool,
    #[serde(default)]
    pub popout_layout_profiles: Vec<PopoutLayoutProfile>,
    #[serde(default)]
    pub active_popout_layout_profile_name: String,
```

with:

```rust
    // Popout window
    #[serde(default)]
    pub popout_open: bool,
    #[serde(default = "default_true")]
    pub popout_show_mm: bool,
    #[serde(default = "default_true")]
    pub popout_show_usbc: bool,
```

- [ ] **Step 3: Update Default impl**

In `impl Default for AppConfiguration`, replace (lines 310-316):

```rust
            multimeter_popout_mode: PopoutDisplayMode::Mini,
            usbc_popout_mode: PopoutDisplayMode::Mini,
            multimeter_popout_frame: None,
            usbc_popout_frame: None,
            popout_alarm_emphasis_enabled: false,
            popout_layout_profiles: Vec::new(),
            active_popout_layout_profile_name: String::new(),
```

with:

```rust
            popout_open: false,
            popout_show_mm: true,
            popout_show_usbc: true,
```

- [ ] **Step 4: Update Inner struct in Deserialize impl**

In the `Inner` struct inside the `Deserialize` impl, replace (lines 426-437):

```rust
            #[serde(default)]
            multimeter_popout_mode: PopoutDisplayMode,
            #[serde(default)]
            usbc_popout_mode: PopoutDisplayMode,
            multimeter_popout_frame: Option<PopoutWindowFrame>,
            usbc_popout_frame: Option<PopoutWindowFrame>,
            #[serde(default)]
            popout_alarm_emphasis_enabled: bool,
            #[serde(default)]
            popout_layout_profiles: Vec<PopoutLayoutProfile>,
            #[serde(default)]
            active_popout_layout_profile_name: String,
```

with:

```rust
            #[serde(default)]
            popout_open: bool,
            #[serde(default = "default_true")]
            popout_show_mm: bool,
            #[serde(default = "default_true")]
            popout_show_usbc: bool,
```

- [ ] **Step 5: Update field mapping in Deserialize impl**

Replace the mapping lines (lines 480-486):

```rust
            multimeter_popout_mode: inner.multimeter_popout_mode,
            usbc_popout_mode: inner.usbc_popout_mode,
            multimeter_popout_frame: inner.multimeter_popout_frame,
            usbc_popout_frame: inner.usbc_popout_frame,
            popout_alarm_emphasis_enabled: inner.popout_alarm_emphasis_enabled,
            popout_layout_profiles: inner.popout_layout_profiles,
            active_popout_layout_profile_name: inner.active_popout_layout_profile_name,
```

with:

```rust
            popout_open: inner.popout_open,
            popout_show_mm: inner.popout_show_mm,
            popout_show_usbc: inner.popout_show_usbc,
```

- [ ] **Step 6: Remove popout_alarm_emphasis checkbox from settings.rs**

In `readout-gui/src/widgets/settings.rs`, remove line 110:

```rust
                        ui.checkbox(&mut self.draft.popout_alarm_emphasis_enabled, "Popout alarm emphasis");
```

- [ ] **Step 7: Compile check**

Run: `cargo check -p readout-persistence && cargo check -p readout-gui`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/readout-persistence/src/config.rs readout-gui/src/widgets/settings.rs
git commit -m "refactor: replace unused popout config types with combined popout fields"
```

---

### Task 3: Rewrite popout.rs — combined popout window

**Files:**
- Rewrite: `readout-gui/src/popout.rs`

- [ ] **Step 1: Write the complete popout.rs**

Replace the entire contents of `readout-gui/src/popout.rs` with:

```rust
use crate::theme::{self, colors};
use crate::widgets::chart::RANGE_OPTIONS;
use readout_core::dashboard_state::{UsbCMetric, USBC_METRICS};
use readout_core::types::{AlarmState, ConnectionState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;

pub struct PopoutState {
    pub open: bool,
    pub show_mm: bool,
    pub show_usbc: bool,
}

impl Default for PopoutState {
    fn default() -> Self {
        Self {
            open: false,
            show_mm: true,
            show_usbc: true,
        }
    }
}

#[derive(Default)]
pub enum PopoutAction {
    #[default]
    None,
    TogglePause,
    TogglePcBeep,
    ToggleMeterBeep,
    ResetEnergy,
    SetUsbcMetric(UsbCMetric),
    SetTimeRange(usize),
}

pub struct PopoutInput {
    pub mm_measurement: Option<DeviceMeasurement>,
    pub usbc_measurement: Option<DeviceMeasurement>,
    pub mm_connection: ConnectionState,
    pub usbc_connection: ConnectionState,
    pub mm_alarm: AlarmState,
    pub usbc_alarm: AlarmState,
    pub mm_chart_data: Vec<[f64; 2]>,
    pub usbc_chart_data: Vec<[f64; 2]>,
    pub paused: bool,
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub usbc_metric: UsbCMetric,
    pub selected_range_idx: usize,
}

fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("popout_combined")
}

pub fn show_combined_popout(
    ctx: &egui::Context,
    state: &mut PopoutState,
    input: PopoutInput,
) -> PopoutAction {
    if !state.open {
        return PopoutAction::None;
    }

    let mut action = PopoutAction::None;

    ctx.show_viewport_immediate(
        viewport_id(),
        egui::ViewportBuilder::default()
            .with_title("readout")
            .with_inner_size([320.0, 500.0])
            .with_min_inner_size([280.0, 300.0])
            .with_always_on_top(),
        |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                state.open = false;
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                render_toolbar(ui, state, &input, &mut action);
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if state.show_mm {
                        render_device_section(
                            ui,
                            DeviceId::Multimeter,
                            input.mm_measurement.as_ref(),
                            &input.mm_connection,
                            input.mm_alarm,
                            &input.mm_chart_data,
                            &mut action,
                        );
                    }

                    if state.show_mm && state.show_usbc {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }

                    if state.show_usbc {
                        render_device_section(
                            ui,
                            DeviceId::UsbC,
                            input.usbc_measurement.as_ref(),
                            &input.usbc_connection,
                            input.usbc_alarm,
                            &input.usbc_chart_data,
                            &mut action,
                        );
                    }
                });
            });
        },
    );

    action
}

fn render_toolbar(
    ui: &mut egui::Ui,
    state: &mut PopoutState,
    input: &PopoutInput,
    action: &mut PopoutAction,
) {
    // Row 1: device visibility + pause + beep
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        if ui
            .selectable_label(state.show_mm, egui::RichText::new("MM").size(10.0))
            .clicked()
        {
            if state.show_mm {
                if state.show_usbc {
                    state.show_mm = false;
                }
            } else {
                state.show_mm = true;
            }
        }
        if ui
            .selectable_label(state.show_usbc, egui::RichText::new("USB-C").size(10.0))
            .clicked()
        {
            if state.show_usbc {
                if state.show_mm {
                    state.show_usbc = false;
                }
            } else {
                state.show_usbc = true;
            }
        }

        ui.separator();

        let pause_label = if input.paused { "▶" } else { "⏸" };
        if ui
            .button(egui::RichText::new(pause_label).size(10.0))
            .clicked()
        {
            *action = PopoutAction::TogglePause;
        }

        ui.separator();

        let pc_icon = if input.pc_beep_enabled { "🔊" } else { "🔇" };
        if ui
            .selectable_label(
                input.pc_beep_enabled,
                egui::RichText::new(format!("{pc_icon} PC")).size(10.0),
            )
            .clicked()
        {
            *action = PopoutAction::TogglePcBeep;
        }
        let meter_icon = if input.meter_beep_enabled {
            "🔔"
        } else {
            "🔇"
        };
        if ui
            .selectable_label(
                input.meter_beep_enabled,
                egui::RichText::new(format!("{meter_icon} M")).size(10.0),
            )
            .clicked()
        {
            *action = PopoutAction::ToggleMeterBeep;
        }
    });

    // Row 2: USB-C metric + time range
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;

        for (metric, label) in USBC_METRICS {
            let selected = input.usbc_metric == *metric;
            if ui
                .selectable_label(selected, egui::RichText::new(*label).size(10.0))
                .clicked()
            {
                *action = PopoutAction::SetUsbcMetric(*metric);
            }
        }

        ui.separator();

        for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
            let selected = i == input.selected_range_idx;
            if ui
                .selectable_label(selected, egui::RichText::new(*label).size(10.0))
                .clicked()
            {
                *action = PopoutAction::SetTimeRange(i);
            }
        }
    });
}

fn render_device_section(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    connection: &ConnectionState,
    alarm: AlarmState,
    chart_data: &[[f64; 2]],
    action: &mut PopoutAction,
) {
    let title = match device {
        DeviceId::Multimeter => "Multimeter",
        DeviceId::UsbC => "USB-C",
    };

    let base = ui.visuals().widgets.noninteractive.bg_fill;
    let fill = match alarm {
        AlarmState::HighAlarm | AlarmState::LowAlarm => theme::tint(base, 200, 50, 50, 0.12),
        AlarmState::Short => theme::tint(base, 210, 120, 10, 0.12),
        AlarmState::Open => theme::tint(base, 180, 170, 20, 0.12),
        AlarmState::None => base,
    };

    egui::Frame::new()
        .fill(fill)
        .inner_margin(8.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            // Title + LED
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .size(11.0)
                        .color(theme::text_secondary(ui)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    connection_led(ui, connection);
                });
            });

            ui.add_space(4.0);

            if let Some(m) = measurement {
                // Primary value
                let value_text = m
                    .primary_value
                    .map(|v| format_si(v, &m.primary_unit))
                    .unwrap_or_else(|| format!("OL {}", m.primary_unit));

                ui.label(
                    egui::RichText::new(&value_text)
                        .size(28.0)
                        .strong()
                        .family(egui::FontFamily::Monospace),
                );

                ui.label(
                    egui::RichText::new(&m.mode_string)
                        .size(10.0)
                        .color(theme::text_secondary(ui)),
                );

                // USB-C secondary values + energy reset
                if device == DeviceId::UsbC {
                    ui.add_space(4.0);
                    if let (Some(current), Some(power)) = (m.secondary_value, m.power_watts) {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format_si(current, "A"))
                                    .size(14.0)
                                    .family(egui::FontFamily::Monospace),
                            );
                            ui.label(
                                egui::RichText::new("|")
                                    .size(14.0)
                                    .color(theme::text_secondary(ui)),
                            );
                            ui.label(
                                egui::RichText::new(format_si(power, "W"))
                                    .size(14.0)
                                    .family(egui::FontFamily::Monospace),
                            );
                        });
                    }
                    if let Some(mwh) = m.energy_mwh {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{mwh:.1} mWh"))
                                    .size(11.0)
                                    .color(theme::text_secondary(ui)),
                            );
                            if ui
                                .small_button("↺")
                                .on_hover_text("Reset energy counter")
                                .clicked()
                            {
                                *action = PopoutAction::ResetEnergy;
                            }
                        });
                    }
                }

                // Alarm badge
                show_alarm_badge(ui, alarm);
            } else {
                ui.label(
                    egui::RichText::new("---")
                        .size(28.0)
                        .family(egui::FontFamily::Monospace)
                        .color(theme::text_secondary(ui)),
                );
            }

            // Mini chart
            ui.add_space(4.0);
            let line_color = match device {
                DeviceId::Multimeter => colors::MM_LINE,
                DeviceId::UsbC => colors::USBC_LINE,
            };
            let chart_id = match device {
                DeviceId::Multimeter => "popout_mm_chart",
                DeviceId::UsbC => "popout_usbc_chart",
            };
            egui_plot::Plot::new(chart_id)
                .height(80.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .show_axes([false, false])
                .show(ui, |plot_ui| {
                    if !chart_data.is_empty() {
                        plot_ui.line(
                            egui_plot::Line::new(title, chart_data.to_vec())
                                .stroke(egui::Stroke::new(1.5, line_color)),
                        );
                    }
                });
        });
}

fn connection_led(ui: &mut egui::Ui, state: &ConnectionState) {
    let color = match state {
        ConnectionState::Connected => colors::CONNECTED,
        ConnectionState::Connecting | ConnectionState::Reconnecting => colors::CONNECTING,
        ConnectionState::Disconnected => colors::DISCONNECTED,
        ConnectionState::Error(_) => colors::ERROR,
    };

    let (rect, response) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        if matches!(state, ConnectionState::Connected) {
            ui.painter()
                .circle_filled(rect.center(), 5.0, theme::with_alpha(color, 25));
        }
        ui.painter().circle_filled(rect.center(), 3.0, color);
    }
    if let ConnectionState::Error(msg) = state {
        response.on_hover_text(msg);
    }
}

fn show_alarm_badge(ui: &mut egui::Ui, alarm: AlarmState) {
    let (icon, text, color) = match alarm {
        AlarmState::HighAlarm => ("▲", "HIGH", colors::ALARM_RED),
        AlarmState::LowAlarm => ("▼", "LOW", colors::ALARM_RED),
        AlarmState::Short => ("⚡", "SHORT", colors::ALARM_ORANGE),
        AlarmState::Open => ("○", "OPEN", colors::ALARM_YELLOW),
        AlarmState::None => return,
    };
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!("{icon} {text}"))
            .size(11.0)
            .strong()
            .color(color),
    );
}
```

- [ ] **Step 2: Compile check (may fail — app.rs still uses old API)**

Run: `cargo check -p readout-gui 2>&1 | head -20`
Expected: Errors about `show_popouts` not found (app.rs still calls old function). This is OK — will be fixed in Task 5.

---

### Task 4: Update header.rs — single popout toggle

**Files:**
- Modify: `readout-gui/src/widgets/header.rs`

- [ ] **Step 1: Simplify HeaderAction and HeaderState**

In `readout-gui/src/widgets/header.rs`, replace the `HeaderAction` enum:

```rust
pub enum HeaderAction {
    None,
    Stop,
    OpenSettings,
    TogglePcBeep,
    ToggleMeterBeep,
    ToggleLog,
    TogglePopoutMM,
    TogglePopoutUsbC,
}
```

with:

```rust
pub enum HeaderAction {
    None,
    Stop,
    OpenSettings,
    TogglePcBeep,
    ToggleMeterBeep,
    ToggleLog,
    TogglePopout,
}
```

Replace `HeaderState`:

```rust
pub struct HeaderState {
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub log_visible: bool,
    pub popout_mm: bool,
    pub popout_usbc: bool,
}
```

with:

```rust
pub struct HeaderState {
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub log_visible: bool,
    pub popout_open: bool,
}
```

- [ ] **Step 2: Replace two popout buttons with single toggle**

In the `show` function, replace the two popout selectable labels:

```rust
        if ui
            .selectable_label(header_state.popout_mm, "⬒ MM")
            .clicked()
        {
            action = HeaderAction::TogglePopoutMM;
        }
        if ui
            .selectable_label(header_state.popout_usbc, "⬒ USB-C")
            .clicked()
        {
            action = HeaderAction::TogglePopoutUsbC;
        }
```

with:

```rust
        if ui
            .selectable_label(header_state.popout_open, "⬒ Popout")
            .clicked()
        {
            action = HeaderAction::TogglePopout;
        }
```

---

### Task 5: Wire up in app.rs — data flow and action handling

**Files:**
- Modify: `readout-gui/src/app.rs`

- [ ] **Step 1: Update PopoutState initialization**

In `ReadOutApp::new`, replace:

```rust
            popout_state: crate::popout::PopoutState::default(),
```

with:

```rust
            popout_state: crate::popout::PopoutState {
                open: config.popout_open,
                show_mm: config.popout_show_mm,
                show_usbc: config.popout_show_usbc,
            },
```

- [ ] **Step 2: Update keyboard shortcuts**

In `handle_keyboard_shortcuts`, replace:

```rust
            // Ctrl+1 / Cmd+1: toggle multimeter popout
            if i.modifiers.command && i.key_pressed(egui::Key::Num1) {
                self.popout_state.multimeter_open = !self.popout_state.multimeter_open;
            }
            // Ctrl+2 / Cmd+2: toggle USB-C popout
            if i.modifiers.command && i.key_pressed(egui::Key::Num2) {
                self.popout_state.usbc_open = !self.popout_state.usbc_open;
            }
```

with:

```rust
            // Ctrl+1 / Cmd+1: toggle popout
            if i.modifiers.command && i.key_pressed(egui::Key::Num1) {
                self.popout_state.open = !self.popout_state.open;
            }
```

- [ ] **Step 3: Replace popout call with combined popout**

Replace the existing popout call (around line 194):

```rust
        // Popout windows
        crate::popout::show_popouts(
            ctx,
            &mut self.popout_state,
            self.state.latest_measurement.get(&DeviceId::Multimeter),
            self.state.latest_measurement.get(&DeviceId::UsbC),
        );
```

with:

```rust
        // Combined popout window
        let popout_action = {
            use std::time::Duration;

            let (range, _) =
                crate::widgets::chart::RANGE_OPTIONS[self.chart_state.selected_range_idx];
            let target_points = 200;

            let now = self
                .state
                .chart_pipelines
                .values()
                .filter_map(|p| p.latest_timestamp())
                .chain(
                    self.state
                        .usbc_chart_pipelines
                        .values()
                        .filter_map(|p| p.latest_timestamp()),
                )
                .max()
                .unwrap_or(Duration::ZERO);

            let mm_chart_data: Vec<[f64; 2]> = self
                .state
                .chart_pipelines
                .get_mut(&DeviceId::Multimeter)
                .map(|p| {
                    p.query_with_now(range, target_points, now)
                        .iter()
                        .map(|(t, v)| [t.as_secs_f64(), *v])
                        .collect()
                })
                .unwrap_or_default();

            let usbc_chart_data: Vec<[f64; 2]> = self
                .state
                .usbc_chart_pipelines
                .get_mut(&self.chart_state.usbc_metric)
                .map(|p| {
                    p.query_with_now(range, target_points, now)
                        .iter()
                        .map(|(t, v)| [t.as_secs_f64(), *v])
                        .collect()
                })
                .unwrap_or_default();

            let input = crate::popout::PopoutInput {
                mm_measurement: self
                    .state
                    .latest_measurement
                    .get(&DeviceId::Multimeter)
                    .cloned(),
                usbc_measurement: self
                    .state
                    .latest_measurement
                    .get(&DeviceId::UsbC)
                    .cloned(),
                mm_connection: self.state.connection_for(DeviceId::Multimeter).clone(),
                usbc_connection: self.state.connection_for(DeviceId::UsbC).clone(),
                mm_alarm: self.state.alarm_for(DeviceId::Multimeter),
                usbc_alarm: self.state.alarm_for(DeviceId::UsbC),
                mm_chart_data,
                usbc_chart_data,
                paused: self.state.paused,
                pc_beep_enabled: self.config.dashboard_beep_master_enabled,
                meter_beep_enabled: self.config.beep_on_short_meter,
                usbc_metric: self.chart_state.usbc_metric,
                selected_range_idx: self.chart_state.selected_range_idx,
            };

            crate::popout::show_combined_popout(ctx, &mut self.popout_state, input)
        };
```

- [ ] **Step 4: Handle PopoutAction**

Insert the following after the popout call block (before the wizard/settings section):

```rust
        match popout_action {
            crate::popout::PopoutAction::TogglePause => {
                self.state.paused = !self.state.paused;
            }
            crate::popout::PopoutAction::TogglePcBeep => {
                self.config.dashboard_beep_master_enabled =
                    !self.config.dashboard_beep_master_enabled;
                let path = self.config_path.clone();
                let config = self.config.clone();
                std::thread::spawn(move || {
                    let _ = readout_persistence::config_store::save(&config, &path);
                });
            }
            crate::popout::PopoutAction::ToggleMeterBeep => {
                self.config.beep_on_short_meter = !self.config.beep_on_short_meter;
                if let Some(ref runtime) = self.runtime {
                    runtime.meter_beep_flag.store(
                        self.config.beep_on_short_meter,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                let path = self.config_path.clone();
                let config = self.config.clone();
                std::thread::spawn(move || {
                    let _ = readout_persistence::config_store::save(&config, &path);
                });
            }
            crate::popout::PopoutAction::ResetEnergy => {
                if let Some(ref runtime) = self.runtime {
                    let _ = runtime
                        .command_tx
                        .try_send(Command::ResetEnergy { device: DeviceId::UsbC });
                }
            }
            crate::popout::PopoutAction::SetUsbcMetric(metric) => {
                self.chart_state.usbc_metric = metric;
            }
            crate::popout::PopoutAction::SetTimeRange(idx) => {
                self.chart_state.selected_range_idx = idx;
            }
            crate::popout::PopoutAction::None => {}
        }
```

- [ ] **Step 5: Update HeaderState construction**

Replace:

```rust
        let header_state = widgets::header::HeaderState {
            pc_beep_enabled: self.config.dashboard_beep_master_enabled,
            meter_beep_enabled: self.config.beep_on_short_meter,
            log_visible: self.show_log_panel,
            popout_mm: self.popout_state.multimeter_open,
            popout_usbc: self.popout_state.usbc_open,
        };
```

with:

```rust
        let header_state = widgets::header::HeaderState {
            pc_beep_enabled: self.config.dashboard_beep_master_enabled,
            meter_beep_enabled: self.config.beep_on_short_meter,
            log_visible: self.show_log_panel,
            popout_open: self.popout_state.open,
        };
```

- [ ] **Step 6: Update HeaderAction handling for popout**

Replace:

```rust
            widgets::header::HeaderAction::TogglePopoutMM => {
                self.popout_state.multimeter_open = !self.popout_state.multimeter_open;
            }
            widgets::header::HeaderAction::TogglePopoutUsbC => {
                self.popout_state.usbc_open = !self.popout_state.usbc_open;
            }
```

with:

```rust
            widgets::header::HeaderAction::TogglePopout => {
                self.popout_state.open = !self.popout_state.open;
            }
```

- [ ] **Step 7: Sync popout state to config on change**

At the end of the `update` method (before `ctx.request_repaint_after`), add:

```rust
        // Sync popout state to config for persistence
        if self.config.popout_open != self.popout_state.open
            || self.config.popout_show_mm != self.popout_state.show_mm
            || self.config.popout_show_usbc != self.popout_state.show_usbc
        {
            self.config.popout_open = self.popout_state.open;
            self.config.popout_show_mm = self.popout_state.show_mm;
            self.config.popout_show_usbc = self.popout_state.show_usbc;
            let path = self.config_path.clone();
            let config = self.config.clone();
            std::thread::spawn(move || {
                let _ = readout_persistence::config_store::save(&config, &path);
            });
        }
```

- [ ] **Step 8: Compile check**

Run: `cargo check -p readout-gui`
Expected: PASS — all references to old API replaced

- [ ] **Step 9: Commit**

```bash
git add readout-gui/src/popout.rs readout-gui/src/widgets/header.rs readout-gui/src/app.rs
git commit -m "feat(gui): combined popout window with mini charts and full controls"
```

---

### Task 6: Final verification

- [ ] **Step 1: Full build**

Run: `cargo build -p readout-gui`
Expected: PASS with no errors

- [ ] **Step 2: Run with simulator and verify**

Run: `cargo run -p readout-gui -- --simulator`

Verify:
- Header shows single "⬒ Popout" button
- Cmd+1 toggles the combined popout
- Popout shows toolbar with MM/USB-C toggles, pause, beep, metric, range
- Both device sections visible with measurement + mini chart
- Toggling MM/USB-C visibility works (at least one must stay on)
- Changing metric/range in popout reflects in main window and vice versa
- Energy reset button appears in USB-C section
- Pause/beep toggles work from popout
- Closing popout via OS close button works
- Reopening popout restores device visibility state
