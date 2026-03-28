# TUI Feature Parity — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring readout-tui to full feature parity with readout-gui — meter control, extended settings, log overlay, device visibility, energy reset.

**Architecture:** The TUI uses a screen-state model (dashboard / settings / meter control / log). New features add screens toggled by keyboard shortcuts, sending `Command::Meter(...)` through the existing `command_tx` channel. Settings expand with categorized field sections. Log overlay is a toggleable bottom panel on the dashboard.

**Tech Stack:** Rust, ratatui 0.30, crossterm 0.29, tokio, readout-core/readout-io/readout-persistence

**Reference:** Current TUI is 910 LOC across 7 files. GUI widgets serve as the feature reference.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `readout-tui/src/app.rs` | Modify | Wire command_tx, add screen states, key routing |
| `readout-tui/src/widgets/mod.rs` | Modify | Export new modules |
| `readout-tui/src/widgets/chart.rs` | Modify | Add 30s and 1m time ranges |
| `readout-tui/src/widgets/device_card.rs` | Modify | Show meter state (mode/range/rate), energy reset hint |
| `readout-tui/src/widgets/status_bar.rs` | Modify | Update key hints per screen state |
| `readout-tui/src/widgets/settings.rs` | Modify | Add all missing settings fields with categories |
| `readout-tui/src/widgets/meter_control.rs` | Create | Full meter control screen |
| `readout-tui/src/widgets/log_panel.rs` | Create | Toggleable log overlay panel |

---

## Chunk 1: Command Infrastructure & Chart Ranges

### Task 1: Wire command_tx through TuiApp

**Files:**
- Modify: `readout-tui/src/app.rs`

- [ ] **Step 1: Add command_tx field to TuiApp**

In `TuiApp` struct, add a field to hold the command sender:

```rust
pub struct TuiApp {
    pub state: DashboardState,
    pub chart_state: widgets::chart::TuiChartState,
    pub settings_screen: widgets::settings::TuiSettingsScreen,
    pub config: AppConfiguration,
    pub config_path: std::path::PathBuf,
    pub should_quit: bool,
    pub command_tx: tokio::sync::mpsc::Sender<readout_core::types::Command>,
}
```

Update `new()` to accept and store `command_tx`:

```rust
pub fn new(
    config: AppConfiguration,
    config_path: std::path::PathBuf,
    command_tx: tokio::sync::mpsc::Sender<readout_core::types::Command>,
) -> Self {
    Self {
        state: DashboardState::new(),
        chart_state: widgets::chart::TuiChartState::new(),
        settings_screen: widgets::settings::TuiSettingsScreen::new(&config),
        config,
        config_path,
        should_quit: false,
        command_tx,
    }
}
```

- [ ] **Step 2: Update run() to pass command_tx**

In `run()`, change `_command_tx` to `command_tx` and pass it to `TuiApp::new()`:

```rust
let command_tx = runtime.command_sender();
// ...
let mut app = TuiApp::new(config, config_path, command_tx);
```

- [ ] **Step 3: Add send_command helper to TuiApp**

```rust
pub fn send_meter_command(&self, cmd: readout_core::types::MultimeterCommand) {
    let _ = self.command_tx.try_send(readout_core::types::Command::Meter(cmd));
}

pub fn send_command(&self, cmd: readout_core::types::Command) {
    let _ = self.command_tx.try_send(cmd);
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 5: Commit**

```bash
git add readout-tui/src/app.rs
git commit -m "feat(tui): wire command_tx through TuiApp"
```

---

### Task 2: Add missing chart time ranges

**Files:**
- Modify: `readout-tui/src/widgets/chart.rs`

- [ ] **Step 1: Add 30s and 1m to RANGE_OPTIONS**

Replace the existing `RANGE_OPTIONS` constant:

```rust
const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(30), "30s"),
    (Duration::from_secs(60), "1m"),
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];
```

- [ ] **Step 2: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 3: Commit**

```bash
git add readout-tui/src/widgets/chart.rs
git commit -m "feat(tui): add 30s and 1m chart time ranges"
```

---

### Task 3: Show meter state in multimeter device card

**Files:**
- Modify: `readout-tui/src/widgets/device_card.rs`

- [ ] **Step 1: Add meter state parameters to render function**

Update the `render_device_card` function signature to accept meter state info. Add parameters for mode, range label, rate, and math stats when device is Multimeter:

```rust
pub fn render_device_card(
    frame: &mut Frame,
    area: Rect,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    alarm: AlarmState,
    meter_mode: Option<&str>,       // e.g. "V DC" — None for USB-C
    meter_range: Option<&str>,      // e.g. "5V"
    meter_rate: Option<&str>,       // e.g. "Medium"
    math_info: Option<&str>,        // e.g. "MIN/MAX" or "dB"
)
```

- [ ] **Step 2: Render meter state line below mode string**

After the mode string line for Multimeter, add a compact status line:

```rust
if device == DeviceId::Multimeter {
    if let (Some(mode), Some(range), Some(rate)) = (meter_mode, meter_range, meter_rate) {
        lines.push(Line::from(vec![
            Span::styled(format!("{mode}"), Style::default().fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled(format!("R:{range}"), Style::default().fg(Color::DarkGray)),
            Span::raw(" | "),
            Span::styled(rate, Style::default().fg(Color::DarkGray)),
        ]));
    }
    if let Some(math) = math_info {
        lines.push(Line::from(Span::styled(
            format!("Math: {math}"),
            Style::default().fg(Color::Yellow),
        )));
    }
}
```

- [ ] **Step 3: Update call sites in app.rs**

In `app.rs` `draw()`, pass meter state from `self.state`:

```rust
// For Multimeter card:
let mode_str = format!("{:?}", self.state.meter_mode);
let rate_str = match self.state.meter_rate {
    MultimeterRate::Fast => "Fast",
    MultimeterRate::Medium => "Medium",
    MultimeterRate::Slow => "Slow",
};
let math_str = self.state.meter_math_function.map(|f| match f {
    MathFunction::Null => "REL",
    MathFunction::Average => "MIN/MAX",
    MathFunction::Db => "dB",
    MathFunction::Dbm => "dBm",
});
widgets::device_card::render_device_card(
    frame, mm_area, DeviceId::Multimeter,
    self.state.latest_measurement.get(&DeviceId::Multimeter),
    self.state.alarm_for(DeviceId::Multimeter),
    Some(&mode_str),
    Some(if self.state.meter_range_label.is_empty() { "Auto" } else { &self.state.meter_range_label }),
    Some(rate_str),
    math_str,
);

// For USB-C card — pass None for all meter fields:
widgets::device_card::render_device_card(
    frame, usbc_area, DeviceId::UsbC,
    self.state.latest_measurement.get(&DeviceId::UsbC),
    self.state.alarm_for(DeviceId::UsbC),
    None, None, None, None,
);
```

- [ ] **Step 4: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 5: Commit**

```bash
git add readout-tui/src/widgets/device_card.rs readout-tui/src/app.rs
git commit -m "feat(tui): show meter mode, range, rate, math in device card"
```

---

## Chunk 2: Meter Control Screen

### Task 4: Create meter control screen widget

**Files:**
- Create: `readout-tui/src/widgets/meter_control.rs`
- Modify: `readout-tui/src/widgets/mod.rs`

- [ ] **Step 1: Create the module file and export it**

Add to `readout-tui/src/widgets/mod.rs`:

```rust
pub mod meter_control;
```

- [ ] **Step 2: Define the screen state struct**

Create `readout-tui/src/widgets/meter_control.rs`:

```rust
use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use readout_core::dashboard_state::DashboardState;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::*;

/// Which section the cursor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterSection {
    Mode,
    Range,
    Rate,
    DualDisplay,
    Null,
    DcFilter,
    AutoImpedance,
    MathFunction,
    DbReference,
    TempSensor,
    TempUnit,
    RemoteLock,
    Reset,
}

const SECTIONS: &[MeterSection] = &[
    MeterSection::Mode,
    MeterSection::Range,
    MeterSection::Rate,
    MeterSection::DualDisplay,
    MeterSection::Null,
    MeterSection::DcFilter,
    MeterSection::AutoImpedance,
    MeterSection::MathFunction,
    MeterSection::DbReference,
    MeterSection::TempSensor,
    MeterSection::TempUnit,
    MeterSection::RemoteLock,
    MeterSection::Reset,
];

pub struct TuiMeterControl {
    pub active: bool,
    pub cursor: usize,       // index into visible_sections()
    pub mode_cursor: usize,  // sub-cursor within mode row
    pub db_ref_cursor: usize,
}

impl TuiMeterControl {
    pub fn new() -> Self {
        Self {
            active: false,
            cursor: 0,
            mode_cursor: 0,
            db_ref_cursor: 8, // default 600 Ω (index 8 in DB_REFERENCE_VALUES)
        }
    }
```

- [ ] **Step 3: Add visible_sections() that filters by current mode**

```rust
    /// Returns sections visible for the current meter mode.
    fn visible_sections(&self, state: &DashboardState) -> Vec<MeterSection> {
        let mut sections = vec![
            MeterSection::Mode,
            MeterSection::Range,
            MeterSection::Rate,
            MeterSection::DualDisplay,
            MeterSection::Null,
        ];
        if state.meter_mode == MeasurementMode::DcVoltage {
            sections.push(MeterSection::DcFilter);
            sections.push(MeterSection::AutoImpedance);
        }
        sections.push(MeterSection::MathFunction);
        if matches!(state.meter_math_function, Some(MathFunction::Db) | Some(MathFunction::Dbm)) {
            sections.push(MeterSection::DbReference);
        }
        if state.meter_mode == MeasurementMode::Temperature {
            sections.push(MeterSection::TempSensor);
            sections.push(MeterSection::TempUnit);
        }
        sections.push(MeterSection::RemoteLock);
        sections.push(MeterSection::Reset);
        sections
    }
```

- [ ] **Step 4: Implement handle_key() returning Option<MultimeterCommand>**

```rust
    pub fn handle_key(&mut self, key: KeyCode, state: &DashboardState) -> Option<MultimeterCommand> {
        let sections = self.visible_sections(state);
        if sections.is_empty() { return None; }
        self.cursor = self.cursor.min(sections.len().saturating_sub(1));
        let current = sections[self.cursor];

        match key {
            KeyCode::Esc => {
                self.active = false;
                return None;
            }
            KeyCode::Up => {
                if self.cursor > 0 { self.cursor -= 1; }
                return None;
            }
            KeyCode::Down => {
                if self.cursor + 1 < sections.len() { self.cursor += 1; }
                return None;
            }
            KeyCode::Left => return self.handle_left(current, state),
            KeyCode::Right => return self.handle_right(current, state),
            KeyCode::Enter | KeyCode::Char(' ') => return self.handle_activate(current, state),
            _ => return None,
        }
    }

    fn handle_left(&mut self, section: MeterSection, state: &DashboardState) -> Option<MultimeterCommand> {
        match section {
            MeterSection::Mode => {
                if self.mode_cursor > 0 { self.mode_cursor -= 1; }
                None
            }
            MeterSection::Range => Some(MultimeterCommand::SetRange(MultimeterRange::Manual(1))),
            MeterSection::Rate => {
                let new = match state.meter_rate {
                    MultimeterRate::Medium => MultimeterRate::Fast,
                    MultimeterRate::Slow => MultimeterRate::Medium,
                    MultimeterRate::Fast => MultimeterRate::Fast,
                };
                Some(MultimeterCommand::SetRate(new))
            }
            MeterSection::DbReference => {
                if self.db_ref_cursor > 0 { self.db_ref_cursor -= 1; }
                let ohms = DB_REFERENCE_VALUES[self.db_ref_cursor];
                Some(MultimeterCommand::SetDbReference(DbReference::Ohms(ohms)))
            }
            _ => None,
        }
    }

    fn handle_right(&mut self, section: MeterSection, state: &DashboardState) -> Option<MultimeterCommand> {
        match section {
            MeterSection::Mode => {
                let modes = mode_list();
                if self.mode_cursor + 1 < modes.len() { self.mode_cursor += 1; }
                None
            }
            MeterSection::Range => Some(MultimeterCommand::SetRange(MultimeterRange::Manual(7))),
            MeterSection::Rate => {
                let new = match state.meter_rate {
                    MultimeterRate::Fast => MultimeterRate::Medium,
                    MultimeterRate::Medium => MultimeterRate::Slow,
                    MultimeterRate::Slow => MultimeterRate::Slow,
                };
                Some(MultimeterCommand::SetRate(new))
            }
            MeterSection::DbReference => {
                if self.db_ref_cursor + 1 < DB_REFERENCE_VALUES.len() { self.db_ref_cursor += 1; }
                let ohms = DB_REFERENCE_VALUES[self.db_ref_cursor];
                Some(MultimeterCommand::SetDbReference(DbReference::Ohms(ohms)))
            }
            _ => None,
        }
    }

    fn handle_activate(&mut self, section: MeterSection, state: &DashboardState) -> Option<MultimeterCommand> {
        match section {
            MeterSection::Mode => {
                let modes = mode_list();
                Some(MultimeterCommand::SetMode(modes[self.mode_cursor].0))
            }
            MeterSection::Range => {
                if state.meter_auto_range {
                    Some(MultimeterCommand::SetRange(MultimeterRange::Manual(3)))
                } else {
                    Some(MultimeterCommand::SetRange(MultimeterRange::Auto))
                }
            }
            MeterSection::DualDisplay => Some(MultimeterCommand::SetDualDisplay(!state.meter_dual_display)),
            MeterSection::Null => Some(MultimeterCommand::SetNull(!state.meter_null_enabled)),
            MeterSection::DcFilter => Some(MultimeterCommand::SetDcFilter(!state.meter_dc_filter)),
            MeterSection::AutoImpedance => Some(MultimeterCommand::SetAutoImpedance(!state.meter_auto_impedance)),
            MeterSection::MathFunction => {
                // Cycle: None → Average → Null → Db → Dbm → None
                match state.meter_math_function {
                    None => Some(MultimeterCommand::StartMath(MathFunction::Average)),
                    Some(MathFunction::Average) => Some(MultimeterCommand::StartMath(MathFunction::Null)),
                    Some(MathFunction::Null) => Some(MultimeterCommand::StartMath(MathFunction::Db)),
                    Some(MathFunction::Db) => Some(MultimeterCommand::StartMath(MathFunction::Dbm)),
                    Some(MathFunction::Dbm) => Some(MultimeterCommand::StopMath),
                }
            }
            MeterSection::TempSensor => {
                // Toggle between Kits90 and Pt100
                Some(MultimeterCommand::SetTempSensorType(TempSensorType::Pt100))
            }
            MeterSection::TempUnit => {
                // Cycle C → F → K
                Some(MultimeterCommand::SetTempUnit(TempUnit::Celsius))
            }
            MeterSection::RemoteLock => Some(MultimeterCommand::SetRemoteMode(true)),
            MeterSection::Reset => Some(MultimeterCommand::ResetDevice),
            _ => None,
        }
    }
}
```

- [ ] **Step 5: Add mode_list helper and checkbox helper**

```rust
fn mode_list() -> &'static [(MeasurementMode, &'static str)] {
    &[
        (MeasurementMode::DcVoltage, "V DC"),
        (MeasurementMode::AcVoltage, "V AC"),
        (MeasurementMode::DcCurrent, "A DC"),
        (MeasurementMode::AcCurrent, "A AC"),
        (MeasurementMode::Resistance, "Ω"),
        (MeasurementMode::Capacitance, "Cap"),
        (MeasurementMode::Frequency, "Hz"),
        (MeasurementMode::Diode, "Diod"),
        (MeasurementMode::Continuity, "Cont"),
        (MeasurementMode::Temperature, "Temp"),
        (MeasurementMode::Period, "Per"),
    ]
}

fn checkbox(enabled: bool) -> &'static str {
    if enabled { "[x]" } else { "[ ]" }
}
```

- [ ] **Step 6: Implement draw()**

```rust
    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &DashboardState) {
        let block = Block::default()
            .title(" Meter Control ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = self.visible_sections(state);
        let mut lines: Vec<Line> = Vec::new();

        for (i, &section) in sections.iter().enumerate() {
            let selected = i == self.cursor;
            let marker = if selected { "▸ " } else { "  " };
            let highlight = if selected { Color::Yellow } else { Color::White };

            let line = match section {
                MeterSection::Mode => {
                    let modes = mode_list();
                    let mut spans = vec![Span::styled(format!("{marker}Mode:   "), Style::default().fg(highlight))];
                    for (j, (mode, label)) in modes.iter().enumerate() {
                        let is_current = state.meter_mode == *mode;
                        let is_sub_selected = selected && j == self.mode_cursor;
                        let style = if is_current {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else if is_sub_selected {
                            Style::default().fg(Color::Black).bg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                    Line::from(spans)
                }
                MeterSection::Range => {
                    let label = if state.meter_auto_range {
                        "Auto".to_string()
                    } else if state.meter_range_label.is_empty() {
                        "---".to_string()
                    } else {
                        state.meter_range_label.clone()
                    };
                    Line::from(vec![
                        Span::styled(format!("{marker}Range:  "), Style::default().fg(highlight)),
                        Span::styled("◀ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled(" ▶", Style::default().fg(Color::DarkGray)),
                        Span::raw("  "),
                        Span::styled(
                            if state.meter_auto_range { "[Auto]" } else { " Auto " },
                            if state.meter_auto_range { Style::default().fg(Color::Black).bg(Color::Green) } else { Style::default().fg(Color::DarkGray) },
                        ),
                        Span::styled("  Enter=toggle", Style::default().fg(Color::DarkGray)),
                    ])
                }
                MeterSection::Rate => {
                    let mut spans = vec![Span::styled(format!("{marker}Rate:   "), Style::default().fg(highlight))];
                    for (rate, label) in &[(MultimeterRate::Fast, "Fast"), (MultimeterRate::Medium, "Medium"), (MultimeterRate::Slow, "Slow")] {
                        let is_current = state.meter_rate == *rate;
                        let style = if is_current {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                    spans.push(Span::styled("  ←/→=change", Style::default().fg(Color::DarkGray)));
                    Line::from(spans)
                }
                MeterSection::DualDisplay => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Dual:   "), Style::default().fg(highlight)),
                        Span::styled(checkbox(state.meter_dual_display), Style::default().fg(Color::Cyan)),
                        Span::raw(" Frequency sub-display"),
                    ])
                }
                MeterSection::Null => {
                    Line::from(vec![
                        Span::styled(format!("{marker}NULL:   "), Style::default().fg(highlight)),
                        Span::styled(checkbox(state.meter_null_enabled), Style::default().fg(Color::Cyan)),
                        Span::raw(" Relative measurement"),
                    ])
                }
                MeterSection::DcFilter => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Filter: "), Style::default().fg(highlight)),
                        Span::styled(checkbox(state.meter_dc_filter), Style::default().fg(Color::Cyan)),
                        Span::raw(" DC filter"),
                    ])
                }
                MeterSection::AutoImpedance => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Auto Z: "), Style::default().fg(highlight)),
                        Span::styled(checkbox(state.meter_auto_impedance), Style::default().fg(Color::Cyan)),
                        Span::raw(" Auto impedance"),
                    ])
                }
                MeterSection::MathFunction => {
                    let label = match state.meter_math_function {
                        None => "Off",
                        Some(MathFunction::Average) => "MIN/MAX",
                        Some(MathFunction::Null) => "REL",
                        Some(MathFunction::Db) => "dB",
                        Some(MathFunction::Dbm) => "dBm",
                    };
                    let mut spans = vec![
                        Span::styled(format!("{marker}Math:   "), Style::default().fg(highlight)),
                        Span::styled(format!("[{label}]"), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled("  Enter=cycle", Style::default().fg(Color::DarkGray)),
                    ];
                    if state.meter_math_function == Some(MathFunction::Average) {
                        if let Some(ref stats) = state.meter_math_stats {
                            spans.push(Span::raw("  "));
                            spans.push(Span::styled(
                                format!("Min:{:.3} Max:{:.3} Avg:{:.3}", stats.min, stats.max, stats.avg),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                    Line::from(spans)
                }
                MeterSection::DbReference => {
                    let ohms = DB_REFERENCE_VALUES[self.db_ref_cursor];
                    Line::from(vec![
                        Span::styled(format!("{marker}dB Ref: "), Style::default().fg(highlight)),
                        Span::styled("◀ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{ohms} Ω"), Style::default().fg(Color::Cyan)),
                        Span::styled(" ▶", Style::default().fg(Color::DarkGray)),
                    ])
                }
                MeterSection::TempSensor => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Sensor: "), Style::default().fg(highlight)),
                        Span::raw("KITS90 / PT100  Enter=set"),
                    ])
                }
                MeterSection::TempUnit => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Unit:   "), Style::default().fg(highlight)),
                        Span::raw("°C / °F / K  Enter=set"),
                    ])
                }
                MeterSection::RemoteLock => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Remote: "), Style::default().fg(highlight)),
                        Span::styled("Enter=Lock panel", Style::default().fg(Color::DarkGray)),
                    ])
                }
                MeterSection::Reset => {
                    Line::from(vec![
                        Span::styled(format!("{marker}Reset:  "), Style::default().fg(highlight)),
                        Span::styled("Enter=Reset device", Style::default().fg(Color::Red)),
                    ])
                }
            };

            lines.push(line);
        }

        // Footer
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            " [Esc] Back  [↑/↓] Navigate  [Enter/Space] Activate  [←/→] Adjust",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }
```

- [ ] **Step 7: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 8: Commit**

```bash
git add readout-tui/src/widgets/meter_control.rs readout-tui/src/widgets/mod.rs
git commit -m "feat(tui): add meter control screen widget"
```

---

### Task 5: Integrate meter control screen into app

**Files:**
- Modify: `readout-tui/src/app.rs`

- [ ] **Step 1: Add screen state enum and meter_control field**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Settings,
    MeterControl,
}
```

Add to `TuiApp`:

```rust
pub screen: Screen,
pub meter_control: widgets::meter_control::TuiMeterControl,
```

Initialize in `new()`:

```rust
screen: Screen::Dashboard,
meter_control: widgets::meter_control::TuiMeterControl::new(),
```

- [ ] **Step 2: Route keys by screen state**

Replace the current `handle_key` method to route by `self.screen`:

```rust
pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Option<AppConfiguration> {
    match self.screen {
        Screen::Dashboard => self.handle_dashboard_key(code, modifiers),
        Screen::Settings => self.handle_settings_key(code, modifiers),
        Screen::MeterControl => {
            self.handle_meter_control_key(code);
            None
        }
    }
}
```

Move existing dashboard key handling into `handle_dashboard_key()`, add `'c'` key:

```rust
fn handle_dashboard_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> Option<AppConfiguration> {
    match code {
        KeyCode::Char('q') => self.should_quit = true,
        KeyCode::Char('p') => self.state.paused = !self.state.paused,
        KeyCode::Char('s') => {
            self.settings_screen.open(&self.config);
            self.screen = Screen::Settings;
        }
        KeyCode::Char('c') => {
            self.meter_control.active = true;
            self.screen = Screen::MeterControl;
        }
        KeyCode::Char('m') => self.chart_state.next_usbc_metric(),
        KeyCode::Right => self.chart_state.next_range(),
        KeyCode::Left => self.chart_state.prev_range(),
        _ => {}
    }
    None
}
```

Move existing settings handling into `handle_settings_key()`, updating screen on close:

```rust
fn handle_settings_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> Option<AppConfiguration> {
    // ... existing settings key handling ...
    // When settings closes:
    if !self.settings_screen.active {
        self.screen = Screen::Dashboard;
    }
    // return saved config if applicable
}
```

Add meter control key handler:

```rust
fn handle_meter_control_key(&mut self, code: KeyCode) {
    if let Some(cmd) = self.meter_control.handle_key(code, &self.state) {
        self.send_meter_command(cmd);
    }
    if !self.meter_control.active {
        self.screen = Screen::Dashboard;
    }
}
```

- [ ] **Step 3: Update draw() to render by screen**

```rust
pub fn draw(&mut self, frame: &mut Frame) {
    match self.screen {
        Screen::Dashboard => self.draw_dashboard(frame),
        Screen::Settings => self.settings_screen.draw(frame, frame.area()),
        Screen::MeterControl => self.meter_control.draw(frame, frame.area(), &self.state),
    }
}
```

Move existing dashboard drawing code into `draw_dashboard()`.

- [ ] **Step 4: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 5: Commit**

```bash
git add readout-tui/src/app.rs
git commit -m "feat(tui): integrate meter control screen with 'c' key"
```

---

## Chunk 3: Extended Settings

### Task 6: Expand settings with all missing fields

**Files:**
- Modify: `readout-tui/src/widgets/settings.rs`

- [ ] **Step 1: Add FieldKind::F32 variant**

Add a new variant to support float fields (alarms, volume):

```rust
enum FieldKind {
    Bool(fn(&mut AppConfiguration) -> &mut bool),
    U32(fn(&mut AppConfiguration) -> &mut u32),
    F32(fn(&mut AppConfiguration) -> &mut f32),
    String(fn(&mut AppConfiguration) -> &mut String),
}
```

Update `apply_edit()` to handle F32:

```rust
FieldKind::F32(accessor) => {
    if let Ok(v) = self.fields[self.selected].value.parse::<f32>() {
        *accessor(&mut self.draft) = v;
    }
}
```

Update `is_toggle()` / field behavior: F32 fields enter edit mode like U32/String.

- [ ] **Step 2: Add section separator support**

Add a `Separator` variant or a label field to SettingsField to render section headers:

```rust
pub struct SettingsField {
    pub label: &'static str,
    pub value: String,
    field_kind: FieldKindOrSeparator,
}

enum FieldKindOrSeparator {
    Field(FieldKind),
    Separator, // renders as a section header, not editable
}
```

When rendering, separators appear as colored header lines (e.g., `── Devices ──`). Skip separators when navigating.

- [ ] **Step 3: Add all missing settings fields**

Expand the `fields` vec in `open()` to include all settings organized by category:

```
── Devices ──
  Simulator mode          Bool
  Multimeter enabled      Bool
  Multimeter port         String
  MM auto-reconnect       Bool
  USB-C enabled           Bool
  USB-C port              String
  USB-C auto-reconnect    Bool
  Sample rate (Hz)        U32

── Display ──
  Graph history (sec)     U32
  Device visibility       String  (Both/Multimeter/UsbC — cycle on Enter)
  Log capture             Bool

── Alarms ──
  DCV high alarm          Bool
  DCV high value          F32
  DCV low alarm           Bool
  DCV low value           F32
  Short threshold (Ω)     F32
  Beep on alarm           Bool
  Beep on short (PC)      Bool
  Beep on short (meter)   Bool
  Beep volume             F32

── CSV Logging ──
  MM CSV logging          Bool
  MM CSV file             String
  USB-C CSV logging       Bool
  USB-C CSV file          String

── OBS Output ──
  MM output file          String
  USB-C output file       String
  MM value label          String
  USB-C value label       String
```

Each field maps to the corresponding `AppConfiguration` field accessor.

- [ ] **Step 4: Add scrolling support**

The settings list is now longer than one screen. Add scroll offset:

```rust
pub struct TuiSettingsScreen {
    pub active: bool,
    pub fields: Vec<SettingsField>,
    pub selected: usize,
    pub editing: bool,
    pub scroll_offset: usize,
    draft: AppConfiguration,
}
```

In `draw()`, render only fields from `scroll_offset..scroll_offset+visible_height`. Auto-scroll when `selected` moves out of view.

- [ ] **Step 5: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 6: Commit**

```bash
git add readout-tui/src/widgets/settings.rs
git commit -m "feat(tui): expand settings with alarms, CSV, OBS, display options"
```

---

## Chunk 4: Log Panel & Device Visibility

### Task 7: Create log panel overlay

**Files:**
- Create: `readout-tui/src/widgets/log_panel.rs`
- Modify: `readout-tui/src/widgets/mod.rs`
- Modify: `readout-tui/src/app.rs`

- [ ] **Step 1: Create log_panel.rs**

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::LogLevel;

pub struct TuiLogPanel {
    pub visible: bool,
    pub scroll_offset: usize,
}

impl TuiLogPanel {
    pub fn new() -> Self {
        Self { visible: false, scroll_offset: 0 }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &DashboardState) {
        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = state.log_entries.iter().rev()
            .skip(self.scroll_offset)
            .take(inner.height as usize)
            .map(|entry| {
                let (prefix, color) = match entry.level {
                    LogLevel::Error => ("ERR", Color::Red),
                    LogLevel::Warning => ("WRN", Color::Yellow),
                    LogLevel::Info => ("INF", Color::Green),
                    LogLevel::Debug => ("DBG", Color::DarkGray),
                };
                Line::from(vec![
                    Span::styled(format!("[{prefix}] "), Style::default().fg(color)),
                    Span::raw(&entry.message),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }
}
```

- [ ] **Step 2: Export and integrate**

Add to `widgets/mod.rs`:

```rust
pub mod log_panel;
```

Add `log_panel: widgets::log_panel::TuiLogPanel` to TuiApp struct.

In dashboard key handling, add `'l'` to toggle:

```rust
KeyCode::Char('l') => self.log_panel.toggle(),
```

- [ ] **Step 3: Modify dashboard layout for log panel**

In `draw_dashboard()`, when `self.log_panel.visible`, split the chart area vertically — give bottom 30% to log panel:

```rust
if self.log_panel.visible {
    let chart_and_log = Layout::vertical([
        Constraint::Percentage(70),
        Constraint::Percentage(30),
    ]).split(chart_area);
    // render charts in chart_and_log[0]
    // render log panel in chart_and_log[1]
    self.log_panel.draw(frame, chart_and_log[1], &self.state);
} else {
    // render charts in full chart_area
}
```

- [ ] **Step 4: Add PageUp/PageDown for log scroll when visible**

```rust
KeyCode::PageUp if self.log_panel.visible => self.log_panel.scroll_up(),
KeyCode::PageDown if self.log_panel.visible => self.log_panel.scroll_down(),
```

- [ ] **Step 5: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 6: Commit**

```bash
git add readout-tui/src/widgets/log_panel.rs readout-tui/src/widgets/mod.rs readout-tui/src/app.rs
git commit -m "feat(tui): add toggleable log panel overlay with 'l' key"
```

---

### Task 8: Add device visibility toggle and energy reset

**Files:**
- Modify: `readout-tui/src/app.rs`

- [ ] **Step 1: Add device visibility state**

Add to TuiApp:

```rust
pub show_mm: bool,
pub show_usbc: bool,
```

Initialize both as `true`. Map keys `'1'` and `'2'` to toggle:

```rust
KeyCode::Char('1') => self.show_mm = !self.show_mm,
KeyCode::Char('2') => self.show_usbc = !self.show_usbc,
```

- [ ] **Step 2: Adjust layout based on visibility**

In `draw_dashboard()`, conditionally render device cards and charts:

```rust
let both_visible = self.show_mm && self.show_usbc;
let mm_visible = self.show_mm;
let usbc_visible = self.show_usbc;

// Device cards area: split based on visibility
if both_visible {
    // existing 50/50 split
} else if mm_visible {
    // full width for MM only
} else if usbc_visible {
    // full width for USB-C only
}

// Charts area: same logic
```

- [ ] **Step 3: Add energy reset with 'e' key**

```rust
KeyCode::Char('e') => {
    self.send_command(readout_core::types::Command::ResetEnergy {
        device: DeviceId::UsbC,
    });
}
```

- [ ] **Step 4: Update status bar hints**

Update `status_bar.rs` to show new keys:

```rust
"[q]uit [p]ause [c]ontrol [s]ett [l]og [m]etric [e]reset [1]mm [2]usbc [←/→]range"
```

- [ ] **Step 5: Build and verify**

Run: `cargo check -p readout-tui`

- [ ] **Step 6: Commit**

```bash
git add readout-tui/src/app.rs readout-tui/src/widgets/status_bar.rs
git commit -m "feat(tui): add device visibility toggle, energy reset, updated key hints"
```

---

## Chunk 5: Integration & Final Build

### Task 9: Full build and test

- [ ] **Step 1: Build entire workspace**

Run: `cargo build`

- [ ] **Step 2: Run all tests**

Run: `cargo test`

- [ ] **Step 3: Test with simulator**

Run: `cargo run -p readout-tui -- --simulator`

Verify:
- `c` opens meter control, arrow keys navigate, Enter toggles/activates
- `s` opens expanded settings with all categories
- `l` toggles log panel
- `1`/`2` toggle device visibility
- `e` resets USB-C energy
- `←/→` cycles all 7 chart ranges (30s through 1h)
- Meter state (mode/range/rate/math) shows in MM device card

- [ ] **Step 4: Fix any compilation or visual issues**

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "fix: TUI feature parity integration fixes"
```
