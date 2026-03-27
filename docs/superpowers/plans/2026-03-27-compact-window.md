# Compact Single-Window App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the dual-window architecture (main window + popout) with a single compact window that serves as the entire application.

**Architecture:** The current popout rendering functions become the main app render loop in a single eframe viewport. Old main-window code (header, device cards, large chart, status strip, log panel) is deleted. Settings and logs are shown as egui::Window overlays. New toolbar and device section widgets are created from the popout code.

**Tech Stack:** Rust, egui 0.33, egui_plot 0.34, eframe 0.33

---

### Task 1: Config changes

**Files:**
- Modify: `crates/readout-persistence/src/config.rs`

- [ ] **Step 1: Update config fields**

In the `AppConfiguration` struct, the `Inner` struct (in custom Deserialize impl), the `Default` impl, and the field mapping, make these changes:

**Remove** field `popout_open: bool` (no longer needed — app is always open).

**Rename** fields:
- `popout_show_mm` → `show_mm`
- `popout_show_usbc` → `show_usbc`

**Add** new field:
```rust
    #[serde(default = "default_true")]
    pub always_on_top: bool,
```

Apply these changes in all four locations: the `AppConfiguration` struct, the `Inner` struct, the `Default` impl (always_on_top defaults to `true`), and the field mapping in the `Deserialize` impl.

- [ ] **Step 2: Compile check**

Run: `cargo check -p readout-persistence`
Expected: PASS (GUI crate will have errors until later tasks)

- [ ] **Step 3: Commit**

```bash
git add crates/readout-persistence/src/config.rs
git commit -m "refactor(config): rename popout fields, add always_on_top, remove popout_open"
```

---

### Task 2: Create new widget modules

**Files:**
- Create: `readout-gui/src/widgets/toolbar.rs`
- Create: `readout-gui/src/widgets/device_section.rs`
- Create: `readout-gui/src/widgets/log_overlay.rs`

These files are created but not wired into the app yet — they compile independently.

- [ ] **Step 1: Create toolbar.rs**

Create `readout-gui/src/widgets/toolbar.rs`:

```rust
use crate::theme::colors;
use readout_core::dashboard_state::UsbCMetric;

use std::time::Duration;

pub const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

pub struct ToolbarState {
    pub show_mm: bool,
    pub show_usbc: bool,
    pub paused: bool,
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub selected_range_idx: usize,
    pub show_log: bool,
    pub always_on_top: bool,
}

#[derive(Default)]
pub enum ToolbarAction {
    #[default]
    None,
    TogglePause,
    TogglePcBeep,
    ToggleMeterBeep,
    SetTimeRange(usize),
    OpenSettings,
    ToggleLog,
    ToggleAlwaysOnTop,
}

/// Renders the compact toolbar (2 rows). Mutates visibility state directly,
/// returns an action for things that need app-level handling.
pub fn show(ui: &mut egui::Ui, state: &mut ToolbarState) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    // Row 1: device visibility + pause + beep + time range
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // Device visibility
        if ui
            .selectable_label(state.show_mm, egui::RichText::new("MM").size(10.0))
            .clicked()
        {
            if !state.show_mm || state.show_usbc {
                state.show_mm = !state.show_mm;
            }
        }
        if ui
            .selectable_label(state.show_usbc, egui::RichText::new("USB-C").size(10.0))
            .clicked()
        {
            if !state.show_usbc || state.show_mm {
                state.show_usbc = !state.show_usbc;
            }
        }

        ui.separator();

        let pause_label = if state.paused { "▶" } else { "⏸" };
        if ui
            .button(egui::RichText::new(pause_label).size(10.0))
            .clicked()
        {
            action = ToolbarAction::TogglePause;
        }

        ui.separator();

        let pc_icon = if state.pc_beep_enabled { "🔊" } else { "🔇" };
        if ui
            .selectable_label(
                state.pc_beep_enabled,
                egui::RichText::new(format!("{pc_icon} PC")).size(10.0),
            )
            .clicked()
        {
            action = ToolbarAction::TogglePcBeep;
        }
        let meter_icon = if state.meter_beep_enabled { "🔔" } else { "🔇" };
        if ui
            .selectable_label(
                state.meter_beep_enabled,
                egui::RichText::new(format!("{meter_icon} M")).size(10.0),
            )
            .clicked()
        {
            action = ToolbarAction::ToggleMeterBeep;
        }

        ui.separator();

        for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
            let selected = i == state.selected_range_idx;
            if ui
                .selectable_label(selected, egui::RichText::new(*label).size(10.0))
                .clicked()
            {
                action = ToolbarAction::SetTimeRange(i);
            }
        }
    });

    // Row 2: log + settings + always-on-top
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        if ui
            .selectable_label(state.show_log, egui::RichText::new("📋 Log").size(10.0))
            .clicked()
        {
            action = ToolbarAction::ToggleLog;
        }

        if ui
            .button(egui::RichText::new("⚙ Settings").size(10.0))
            .clicked()
        {
            action = ToolbarAction::OpenSettings;
        }

        if ui
            .selectable_label(
                state.always_on_top,
                egui::RichText::new("📌").size(10.0),
            )
            .on_hover_text("Always on top")
            .clicked()
        {
            action = ToolbarAction::ToggleAlwaysOnTop;
        }
    });

    action
}
```

- [ ] **Step 2: Create device_section.rs**

Create `readout-gui/src/widgets/device_section.rs`:

```rust
use crate::theme::{self, colors};
use crate::widgets::toolbar::RANGE_OPTIONS;
use readout_core::chart_pipeline::ChartPipeline;
use readout_core::dashboard_state::{UsbCMetric, USBC_METRICS};
use readout_core::types::{AlarmState, ConnectionState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;
use std::time::Duration;

#[derive(Default)]
pub enum SectionAction {
    #[default]
    None,
    ResetEnergy,
    SetUsbcMetric(UsbCMetric),
}

pub fn show(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    connection: &ConnectionState,
    alarm: AlarmState,
    pipeline: Option<&mut ChartPipeline>,
    selected_range_idx: usize,
    usbc_metric: UsbCMetric,
) -> SectionAction {
    let mut action = SectionAction::None;

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
                                action = SectionAction::ResetEnergy;
                            }
                        });
                    }

                    // USB-C metric selector
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (metric, label) in USBC_METRICS {
                            let selected = usbc_metric == *metric;
                            if ui
                                .selectable_label(
                                    selected,
                                    egui::RichText::new(*label).size(10.0),
                                )
                                .clicked()
                            {
                                action = SectionAction::SetUsbcMetric(*metric);
                            }
                        }
                    });
                }

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
                DeviceId::Multimeter => "mm_chart",
                DeviceId::UsbC => "usbc_chart",
            };

            // Query chart data from pipeline
            let (range, _) = RANGE_OPTIONS[selected_range_idx];
            let target_points = (ui.available_width() as usize).max(100);
            let chart_data: Vec<[f64; 2]> = pipeline
                .map(|p| {
                    let now = p.latest_timestamp().unwrap_or(Duration::ZERO);
                    p.query_with_now(range, target_points, now)
                        .iter()
                        .map(|(t, v)| [t.as_secs_f64(), *v])
                        .collect()
                })
                .unwrap_or_default();

            egui_plot::Plot::new(chart_id)
                .height(80.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .show_axes([false, false])
                .show(ui, |plot_ui| {
                    if !chart_data.is_empty() {
                        plot_ui.line(
                            egui_plot::Line::new(title, chart_data)
                                .stroke(egui::Stroke::new(1.5, line_color)),
                        );
                    }
                });
        });

    action
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

- [ ] **Step 3: Create log_overlay.rs**

Create `readout-gui/src/widgets/log_overlay.rs`:

```rust
use crate::theme::{self, colors};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::LogLevel;

pub fn show(ctx: &egui::Context, state: &DashboardState, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Window::new("Log")
        .open(open)
        .resizable(true)
        .default_width(300.0)
        .default_height(250.0)
        .show(ctx, |ui| {
            // Health metrics header
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Measurements: {} | Errors: {} | Reconnects: {}",
                        state.health.measurement_count,
                        state.health.error_count,
                        state.health.reconnect_count,
                    ))
                    .size(10.0)
                    .color(self::text_secondary_color(ui)),
                );
            });
            ui.separator();

            // Log entries
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if state.log_entries.is_empty() {
                        ui.label(
                            egui::RichText::new("No log entries")
                                .color(self::text_secondary_color(ui))
                                .italics(),
                        );
                        return;
                    }
                    for entry in &state.log_entries {
                        let color = match entry.level {
                            LogLevel::Error => colors::ERROR,
                            LogLevel::Warning => colors::CONNECTING,
                            LogLevel::Info => ui.visuals().widgets.noninteractive.fg_stroke.color,
                            LogLevel::Debug => self::text_secondary_color(ui),
                        };
                        ui.label(
                            egui::RichText::new(&entry.message)
                                .family(egui::FontFamily::Monospace)
                                .size(11.0)
                                .color(color),
                        );
                    }
                });
        });
}

fn text_secondary_color(ui: &egui::Ui) -> egui::Color32 {
    theme::text_secondary(ui)
}
```

- [ ] **Step 4: Compile check**

These files are not wired in yet but should parse correctly. Add them temporarily to `widgets/mod.rs` to verify:

```rust
pub mod device_section;
pub mod first_run_wizard;
pub mod log_overlay;
pub mod settings;
pub mod toolbar;
```

Note: also temporarily keep the old modules so the existing app.rs compiles:

```rust
pub mod chart;
pub mod device_card;
pub mod header;
pub mod log_panel;
pub mod status_strip;
```

Run: `cargo check -p readout-gui`
Expected: May have errors from config field renames (from Task 1). The implementing agent should fix these references in the old code temporarily or do Task 1 and Task 2 together.

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/toolbar.rs readout-gui/src/widgets/device_section.rs readout-gui/src/widgets/log_overlay.rs
git commit -m "feat(gui): add compact toolbar, device section, and log overlay widgets"
```

---

### Task 3: Rewrite app.rs + main.rs + cleanup

This is the big switchover. All changes must happen together for compilation.

**Files:**
- Rewrite: `readout-gui/src/app.rs`
- Modify: `readout-gui/src/main.rs`
- Rewrite: `readout-gui/src/widgets/mod.rs`
- Delete: `readout-gui/src/popout.rs`
- Delete: `readout-gui/src/widgets/header.rs`
- Delete: `readout-gui/src/widgets/device_card.rs`
- Delete: `readout-gui/src/widgets/chart.rs`
- Delete: `readout-gui/src/widgets/status_strip.rs`
- Delete: `readout-gui/src/widgets/log_panel.rs`

- [ ] **Step 1: Rewrite widgets/mod.rs**

Replace entire contents of `readout-gui/src/widgets/mod.rs` with:

```rust
pub mod device_section;
pub mod first_run_wizard;
pub mod log_overlay;
pub mod settings;
pub mod toolbar;
```

- [ ] **Step 2: Delete old files**

Delete these files:
- `readout-gui/src/popout.rs`
- `readout-gui/src/widgets/header.rs`
- `readout-gui/src/widgets/device_card.rs`
- `readout-gui/src/widgets/chart.rs`
- `readout-gui/src/widgets/status_strip.rs`
- `readout-gui/src/widgets/log_panel.rs`

- [ ] **Step 3: Remove popout module from main.rs**

In `readout-gui/src/main.rs`, remove the `mod popout;` line. Also update the window settings:

Replace:
```rust
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
```

with:
```rust
    let always_on_top = config.always_on_top;
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([320.0, 500.0])
        .with_min_inner_size([280.0, 250.0]);
    if always_on_top {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
```

- [ ] **Step 4: Rewrite app.rs**

Replace the entire contents of `readout-gui/src/app.rs` with the following. Key changes:
- `ReadOutApp` no longer has `popout_state`, `show_log_panel` renamed to `show_log`
- `show_mm` and `show_usbc` stored directly in the app struct
- No header/status strip/log panel/device card/chart rendering
- CentralPanel renders toolbar + device sections directly
- Actions handled inline after CentralPanel closure

```rust
use crate::widgets;
use readout_core::dashboard_state::{DashboardState, UsbCMetric};
use readout_core::types::{AlarmState, Command, ConnectionState, DeviceId, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use readout_persistence::config_store;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

struct RuntimeHandle {
    event_rx: std::sync::mpsc::Receiver<RuntimeEvent>,
    command_tx: tokio::sync::mpsc::Sender<Command>,
    cancel: CancellationToken,
    bg_thread: Option<std::thread::JoinHandle<()>>,
    meter_beep_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl RuntimeHandle {
    fn start(config: &AppConfiguration, ctx: &egui::Context) -> Self {
        let (std_tx, std_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();

        let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
        let command_tx = runtime.command_sender();
        let meter_beep_flag = runtime.meter_beep_flag();

        let ctx_clone = ctx.clone();
        let cancel_clone = cancel.clone();
        let bg_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async move {
                let runtime_cancel = cancel_clone.clone();
                let runtime_handle = tokio::spawn(async move {
                    runtime.run(runtime_cancel).await;
                });

                loop {
                    tokio::select! {
                        _ = cancel_clone.cancelled() => break,
                        result = broadcast_rx.recv() => {
                            match result {
                                Ok(event) => {
                                    let _ = std_tx.send(event);
                                    ctx_clone.request_repaint();
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!("GUI lagged {n} events");
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    }
                }

                let _ = runtime_handle.await;
            });
        });

        Self {
            event_rx: std_rx,
            command_tx,
            cancel,
            bg_thread: Some(bg_thread),
            meter_beep_flag,
        }
    }

    fn shutdown(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.bg_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct ReadOutApp {
    runtime: Option<RuntimeHandle>,
    state: DashboardState,
    settings_panel: widgets::settings::SettingsPanel,
    wizard: widgets::first_run_wizard::FirstRunWizard,
    audio: crate::audio::AlarmAudio,
    running: bool,
    show_mm: bool,
    show_usbc: bool,
    show_log: bool,
    always_on_top: bool,
    usbc_metric: UsbCMetric,
    selected_range_idx: usize,
    config: AppConfiguration,
    config_path: PathBuf,
    ctx: egui::Context,
    applied_theme: Option<readout_persistence::config::DashboardTheme>,
}

impl ReadOutApp {
    pub fn new(
        config: AppConfiguration,
        config_path: PathBuf,
        first_run: bool,
        ctx: &egui::Context,
    ) -> Self {
        let runtime = if first_run {
            None
        } else {
            Some(RuntimeHandle::start(&config, ctx))
        };

        Self {
            runtime,
            state: DashboardState::new(),
            settings_panel: widgets::settings::SettingsPanel::new(&config),
            wizard: widgets::first_run_wizard::FirstRunWizard::new(&config, first_run),
            audio: crate::audio::AlarmAudio::new(),
            running: !first_run,
            show_mm: config.show_mm,
            show_usbc: config.show_usbc,
            show_log: false,
            always_on_top: config.always_on_top,
            usbc_metric: UsbCMetric::Voltage,
            selected_range_idx: 0,
            config,
            config_path,
            ctx: ctx.clone(),
            applied_theme: None,
        }
    }

    fn start_runtime(&mut self) {
        if self.runtime.is_some() {
            return;
        }
        self.runtime = Some(RuntimeHandle::start(&self.config, &self.ctx));
        self.running = true;
        self.state = DashboardState::new();
    }

    fn save_config_async(&self) {
        let path = self.config_path.clone();
        let config = self.config.clone();
        std::thread::spawn(move || {
            let _ = config_store::save(&config, &path);
        });
    }
}

impl eframe::App for ReadOutApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.applied_theme != Some(self.config.dashboard_theme) {
            crate::theme::apply_theme(ctx, self.config.dashboard_theme);
            self.applied_theme = Some(self.config.dashboard_theme);
        }

        // Drain runtime events
        if let Some(ref runtime) = self.runtime {
            while let Ok(event) = runtime.event_rx.try_recv() {
                self.state.handle_event(event);
            }
        }

        // Alarm audio
        {
            use readout_core::types::AlarmState;
            let mm_alarm = self.state.alarm_for(DeviceId::Multimeter);
            let should_sound = self.config.dashboard_beep_master_enabled
                && match mm_alarm {
                    AlarmState::Short => self.config.beep_on_short_pc,
                    AlarmState::HighAlarm | AlarmState::LowAlarm => self.config.beep_on_alarm,
                    _ => false,
                };
            self.audio.set_volume(self.config.pc_beep_volume as f32);
            self.audio.set_active(should_sound);
        }

        // Keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::P) {
                self.state.paused = !self.state.paused;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::L) {
                self.show_log = !self.show_log;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Comma) {
                self.settings_panel.open_with(&self.config);
            }
        });

        // Overlays (rendered before CentralPanel so they appear on top)
        if let Some(new_config) = self.wizard.show(ctx) {
            if let Err(e) = config_store::save(&new_config, &self.config_path) {
                tracing::error!("Failed to save wizard config: {e:?}");
            }
            self.config = new_config;
            self.start_runtime();
        }

        if let Some(new_config) = self.settings_panel.show(ctx) {
            if let Err(e) = config_store::save(&new_config, &self.config_path) {
                tracing::error!("Failed to save config: {e:?}");
            }
            self.config = new_config;
        }

        widgets::log_overlay::show(ctx, &self.state, &mut self.show_log);

        // --- Main content ---
        let mut toolbar_action = widgets::toolbar::ToolbarAction::None;
        let mut section_action = widgets::device_section::SectionAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut toolbar_state = widgets::toolbar::ToolbarState {
                show_mm: self.show_mm,
                show_usbc: self.show_usbc,
                paused: self.state.paused,
                pc_beep_enabled: self.config.dashboard_beep_master_enabled,
                meter_beep_enabled: self.config.beep_on_short_meter,
                selected_range_idx: self.selected_range_idx,
                show_log: self.show_log,
                always_on_top: self.always_on_top,
            };

            toolbar_action = widgets::toolbar::show(ui, &mut toolbar_state);

            // Read back visibility changes (mutated directly by toolbar)
            self.show_mm = toolbar_state.show_mm;
            self.show_usbc = toolbar_state.show_usbc;

            ui.separator();

            // NOTE: Use direct field access (not connection_for/alarm_for methods)
            // to avoid borrowing all of DashboardState while chart pipelines are mut-borrowed.
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.show_mm {
                    let default_conn = ConnectionState::Disconnected;
                    let mm_conn = self.state.connection_state
                        .get(&DeviceId::Multimeter)
                        .unwrap_or(&default_conn);
                    let mm_alarm = self.state.alarm_state
                        .get(&DeviceId::Multimeter)
                        .copied()
                        .unwrap_or(AlarmState::None);
                    let mm_pipeline = self.state.chart_pipelines.get_mut(&DeviceId::Multimeter);
                    widgets::device_section::show(
                        ui,
                        DeviceId::Multimeter,
                        self.state.latest_measurement.get(&DeviceId::Multimeter),
                        mm_conn,
                        mm_alarm,
                        mm_pipeline,
                        self.selected_range_idx,
                        self.usbc_metric,
                    );
                }

                if self.show_mm && self.show_usbc {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                }

                if self.show_usbc {
                    let default_conn = ConnectionState::Disconnected;
                    let usbc_conn = self.state.connection_state
                        .get(&DeviceId::UsbC)
                        .unwrap_or(&default_conn);
                    let usbc_alarm = self.state.alarm_state
                        .get(&DeviceId::UsbC)
                        .copied()
                        .unwrap_or(AlarmState::None);
                    let usbc_pipeline =
                        self.state.usbc_chart_pipelines.get_mut(&self.usbc_metric);
                    let sa = widgets::device_section::show(
                        ui,
                        DeviceId::UsbC,
                        self.state.latest_measurement.get(&DeviceId::UsbC),
                        usbc_conn,
                        usbc_alarm,
                        usbc_pipeline,
                        self.selected_range_idx,
                        self.usbc_metric,
                    );
                    if !matches!(sa, widgets::device_section::SectionAction::None) {
                        section_action = sa;
                    }
                }
            });
        });

        // Handle toolbar actions
        match toolbar_action {
            widgets::toolbar::ToolbarAction::TogglePause => {
                self.state.paused = !self.state.paused;
            }
            widgets::toolbar::ToolbarAction::TogglePcBeep => {
                self.config.dashboard_beep_master_enabled =
                    !self.config.dashboard_beep_master_enabled;
                self.save_config_async();
            }
            widgets::toolbar::ToolbarAction::ToggleMeterBeep => {
                self.config.beep_on_short_meter = !self.config.beep_on_short_meter;
                if let Some(ref runtime) = self.runtime {
                    runtime.meter_beep_flag.store(
                        self.config.beep_on_short_meter,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                self.save_config_async();
            }
            widgets::toolbar::ToolbarAction::SetTimeRange(idx) => {
                self.selected_range_idx = idx;
            }
            widgets::toolbar::ToolbarAction::OpenSettings => {
                self.settings_panel.open_with(&self.config);
            }
            widgets::toolbar::ToolbarAction::ToggleLog => {
                self.show_log = !self.show_log;
            }
            widgets::toolbar::ToolbarAction::ToggleAlwaysOnTop => {
                self.always_on_top = !self.always_on_top;
                self.config.always_on_top = self.always_on_top;
                let level = if self.always_on_top {
                    egui::viewport::WindowLevel::AlwaysOnTop
                } else {
                    egui::viewport::WindowLevel::Normal
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                self.save_config_async();
            }
            widgets::toolbar::ToolbarAction::None => {}
        }

        // Handle device section actions
        match section_action {
            widgets::device_section::SectionAction::ResetEnergy => {
                if let Some(ref runtime) = self.runtime {
                    let _ = runtime
                        .command_tx
                        .try_send(Command::ResetEnergy { device: DeviceId::UsbC });
                }
            }
            widgets::device_section::SectionAction::SetUsbcMetric(metric) => {
                self.usbc_metric = metric;
            }
            widgets::device_section::SectionAction::None => {}
        }

        // Persist visibility/top state on change
        if self.config.show_mm != self.show_mm
            || self.config.show_usbc != self.show_usbc
        {
            self.config.show_mm = self.show_mm;
            self.config.show_usbc = self.show_usbc;
            self.save_config_async();
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}
```

- [ ] **Step 5: Compile check**

Run: `cargo check -p readout-gui`
Expected: PASS (or minor fixups needed for borrow checker / API differences)

- [ ] **Step 6: Run and verify**

Run: `cargo run -p readout-gui -- --simulator`
Expected: Single compact window with toolbar, device sections, mini charts, settings overlay, log overlay.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(gui): convert to single compact window, remove main window code"
```

---

### Task 4: Auto-resize animation

**Files:**
- Modify: `readout-gui/src/app.rs`

- [ ] **Step 1: Add resize animation state**

Add fields to `ReadOutApp`:

```rust
    // Add to struct fields:
    target_height: Option<f32>,
    prev_show_mm: bool,
    prev_show_usbc: bool,
```

Initialize in `new()`:

```rust
    target_height: None,
    prev_show_mm: config.show_mm,
    prev_show_usbc: config.show_usbc,
```

- [ ] **Step 2: Add resize logic at end of update()**

Insert before `ctx.request_repaint_after(...)`:

```rust
        // Auto-resize animation on visibility toggle
        if self.show_mm != self.prev_show_mm || self.show_usbc != self.prev_show_usbc {
            self.prev_show_mm = self.show_mm;
            self.prev_show_usbc = self.show_usbc;

            // Estimate target height: toolbar ~50, device section ~250, separator ~12
            let toolbar_h = 50.0_f32;
            let section_h = 260.0_f32;
            let sep_h = 12.0_f32;
            let n_sections = self.show_mm as u8 + self.show_usbc as u8;
            let target = toolbar_h
                + section_h * n_sections as f32
                + if n_sections == 2 { sep_h } else { 0.0 };
            self.target_height = Some(target);
        }

        if let Some(target) = self.target_height {
            let current = ctx.input(|i| i.screen_rect().height());
            let diff = target - current;
            if diff.abs() < 2.0 {
                self.target_height = None;
            } else {
                let new_height = current + diff * 0.15; // lerp ~200ms at 250ms repaint
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    ctx.input(|i| i.screen_rect().width()),
                    new_height,
                )));
            }
        }
```

- [ ] **Step 3: Compile check and verify**

Run: `cargo check -p readout-gui && cargo run -p readout-gui -- --simulator`
Expected: When toggling MM or USB-C off, the window smoothly animates to a smaller height.

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/app.rs
git commit -m "feat(gui): animated window resize on device visibility toggle"
```
