# Meter Control Extended Features — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Meter Control panel with all high and medium priority SCPI features from issue #2 (dual display, HW MIN/MAX/AVG, NULL/REL, DC filter, auto impedance, continuity threshold, temperature config, device reset).

**Architecture:** New `MultimeterCommand` variants drive SCPI commands through the existing command channel. The driver gains new methods for each feature. `MeterState` event carries extended state back to GUI. `DashboardState` stores the new fields. Meter Control UI adds new sections.

**Tech Stack:** Rust, egui, tokio channels, SCPI over serial

**Reference:** https://github.com/vaclavik-xyz/readOutRS/issues/2

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/readout-core/src/types.rs` | Modify | Add new MultimeterCommand variants, extend MeterState event |
| `crates/readout-core/src/dashboard_state.rs` | Modify | Add fields for new meter state (null, dual display, math, etc.) |
| `crates/readout-io/src/multimeter_driver.rs` | Modify | Add SCPI methods for each feature |
| `crates/readout-io/src/runtime.rs` | Modify | Handle new MultimeterCommand variants in handle_meter_command |
| `readout-gui/src/widgets/meter_control.rs` | Modify | Add UI sections for new features |

---

## Chunk 1: Core Types & Driver — Dual Display

### Task 1: Add dual display types and command

**Files:**
- Modify: `crates/readout-core/src/types.rs`
- Modify: `crates/readout-core/src/dashboard_state.rs`

- [ ] **Step 1: Add MultimeterCommand variants for dual display**

In `crates/readout-core/src/types.rs`, add to `MultimeterCommand` enum:

```rust
pub enum MultimeterCommand {
    QueryIdentity,
    SetMode(crate::measurement_mode::MeasurementMode),
    SetRange(MultimeterRange),
    SetRate(MultimeterRate),
    // New:
    SetDualDisplay(bool),        // FUNC2 "FREQuency" / FUNC2 "NONe"
    SetNull(bool),               // VOLT:DC:NULL ON/OFF (mode-dependent)
    SetDcFilter(bool),           // VOLT:DC:FILT ON/OFF
    SetAutoImpedance(bool),      // VOLT:DC:IMP:AUTO ON/OFF
    SetContinuityThreshold(f64), // CONT:THRE <value>
    SetTempSensorType(TempSensorType), // TEMP:RTD:TYPE
    SetTempUnit(TempUnit),       // TEMP:RTD:UNIT
    StartMath(MathFunction),     // CALC:FUNC {NULL|AVERage}
    StopMath,                    // CALC:STAT OFF
    QueryMathStats,              // CALC:AVER:ALL?
    ResetDevice,                 // *RST
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempSensorType {
    Kits90,
    Pt100,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempUnit {
    Celsius,
    Fahrenheit,
    Kelvin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathFunction {
    Null,     // CALC:FUNC NULL (relative)
    Average,  // CALC:FUNC AVERage (min/max/avg tracking)
}
```

- [ ] **Step 2: Extend RuntimeEvent::MeterState with new fields**

In `crates/readout-core/src/types.rs`, update `RuntimeEvent::MeterState`:

```rust
RuntimeEvent::MeterState {
    identity: Option<String>,
    mode: crate::measurement_mode::MeasurementMode,
    range_label: String,
    rate: MultimeterRate,
    auto_range: bool,
    // New:
    dual_display: bool,
    null_enabled: bool,
    dc_filter: bool,
    auto_impedance: bool,
    math_function: Option<MathFunction>,
    math_stats: Option<MathStats>,
},
```

Add `MathStats` struct:

```rust
#[derive(Debug, Clone, Copy)]
pub struct MathStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub count: u32,
}
```

- [ ] **Step 3: Add fields to DashboardState**

In `crates/readout-core/src/dashboard_state.rs`, add to `DashboardState`:

```rust
pub meter_dual_display: bool,
pub meter_null_enabled: bool,
pub meter_dc_filter: bool,
pub meter_auto_impedance: bool,
pub meter_math_function: Option<MathFunction>,
pub meter_math_stats: Option<MathStats>,
```

Initialize all as `false`/`None` in `new()`. Handle new fields in `MeterState` event processing.

- [ ] **Step 4: Build and verify compilation**

Run: `cargo build -p readout-core`

- [ ] **Step 5: Commit**

```bash
git add crates/readout-core/
git commit -m "feat(core): add extended meter control types for dual display, math, null, filters"
```

---

### Task 2: Add SCPI methods to MultimeterDriver

**Files:**
- Modify: `crates/readout-io/src/multimeter_driver.rs`

- [ ] **Step 1: Add dual display methods**

```rust
pub async fn set_dual_display(&mut self, enabled: bool) -> Result<(), TransportError> {
    let cmd = if enabled { "FUNC2 \"FREQuency\"" } else { "FUNC2 \"NONe\"" };
    let _ = self.transport.query(cmd).await?;
    Ok(())
}

pub async fn query_dual_display(&mut self) -> bool {
    self.transport.query("FUNC2?").await
        .ok().flatten()
        .map(|s| !s.trim().to_uppercase().contains("NON"))
        .unwrap_or(false)
}
```

- [ ] **Step 2: Add NULL/REL method**

NULL command depends on current mode. Build the SCPI prefix from current mode:

```rust
pub async fn set_null(&mut self, enabled: bool) -> Result<(), TransportError> {
    let prefix = self.sense_prefix();
    if let Some(p) = prefix {
        let cmd = format!("{p}:NULL {}", if enabled { "ON" } else { "OFF" });
        let _ = self.transport.query(&cmd).await?;
    }
    Ok(())
}

pub async fn query_null(&mut self) -> bool {
    let prefix = self.sense_prefix();
    if let Some(p) = prefix {
        let cmd = format!("{p}:NULL?");
        return self.transport.query(&cmd).await
            .ok().flatten()
            .map(|s| s.trim() == "1" || s.trim().to_uppercase() == "ON")
            .unwrap_or(false);
    }
    false
}

fn sense_prefix(&self) -> Option<&'static str> {
    let mode = MeasurementModeParser::parse(Some(&self.current_mode));
    match mode {
        MeasurementMode::DcVoltage => Some("VOLT:DC"),
        MeasurementMode::AcVoltage => Some("VOLT:AC"),
        MeasurementMode::DcCurrent => Some("CURR:DC"),
        MeasurementMode::AcCurrent => Some("CURR:AC"),
        MeasurementMode::Resistance => Some("RES"),
        MeasurementMode::Capacitance => Some("CAP"),
        MeasurementMode::Temperature => Some("TEMP:RTD"),
        _ => None,
    }
}
```

- [ ] **Step 3: Add DC filter and auto impedance**

```rust
pub async fn set_dc_filter(&mut self, enabled: bool) -> Result<(), TransportError> {
    let cmd = format!("VOLT:DC:FILT {}", if enabled { "ON" } else { "OFF" });
    let _ = self.transport.query(&cmd).await?;
    Ok(())
}

pub async fn set_auto_impedance(&mut self, enabled: bool) -> Result<(), TransportError> {
    let cmd = format!("VOLT:DC:IMP:AUTO {}", if enabled { "ON" } else { "OFF" });
    let _ = self.transport.query(&cmd).await?;
    Ok(())
}
```

- [ ] **Step 4: Add continuity threshold**

```rust
pub async fn set_continuity_threshold(&mut self, ohms: f64) -> Result<(), TransportError> {
    let cmd = format!("CONT:THRE {}", ohms);
    let _ = self.transport.query(&cmd).await?;
    Ok(())
}
```

- [ ] **Step 5: Add temperature config**

```rust
pub async fn set_temp_sensor_type(&mut self, sensor: TempSensorType) -> Result<(), TransportError> {
    let cmd = match sensor {
        TempSensorType::Kits90 => "TEMP:RTD:TYPE KITS90",
        TempSensorType::Pt100 => "TEMP:RTD:TYPE PT100",
    };
    let _ = self.transport.query(cmd).await?;
    Ok(())
}

pub async fn set_temp_unit(&mut self, unit: TempUnit) -> Result<(), TransportError> {
    let cmd = match unit {
        TempUnit::Celsius => "TEMP:RTD:UNIT C",
        TempUnit::Fahrenheit => "TEMP:RTD:UNIT F",
        TempUnit::Kelvin => "TEMP:RTD:UNIT K",
    };
    let _ = self.transport.query(cmd).await?;
    Ok(())
}
```

- [ ] **Step 6: Add math/statistics methods**

```rust
pub async fn start_math(&mut self, func: MathFunction) -> Result<(), TransportError> {
    let cmd = match func {
        MathFunction::Null => "CALC:FUNC NULL",
        MathFunction::Average => "CALC:FUNC AVERage",
    };
    let _ = self.transport.query(cmd).await?;
    Ok(())
}

pub async fn stop_math(&mut self) -> Result<(), TransportError> {
    let _ = self.transport.query("CALC:STAT OFF").await?;
    Ok(())
}

pub async fn query_math_stats(&mut self) -> Option<MathStats> {
    let response = self.transport.query("CALC:AVER:ALL?").await.ok()??;
    let parts: Vec<&str> = response.trim().split(',').collect();
    if parts.len() >= 4 {
        Some(MathStats {
            min: parts[0].trim().parse().ok()?,
            max: parts[1].trim().parse().ok()?,
            avg: parts[2].trim().parse().ok()?,
            count: parts[3].trim().parse().ok()?,
        })
    } else {
        None
    }
}
```

- [ ] **Step 7: Add device reset**

```rust
pub async fn reset_device(&mut self) -> Result<(), TransportError> {
    let _ = self.transport.query("*RST").await?;
    // Re-query state after reset
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    Ok(())
}
```

- [ ] **Step 8: Extend query_state to include new fields**

```rust
pub async fn query_state(&mut self) -> MeterStateSnapshot {
    // ... existing mode/range/auto/rate queries ...

    let dual_display = self.query_dual_display().await;
    let null_enabled = self.query_null().await;

    MeterStateSnapshot {
        mode, range_label, rate, auto_range,
        dual_display,
        null_enabled,
        dc_filter: false,       // no query command available
        auto_impedance: false,   // no query command available
        math_function: None,     // tracked client-side
        math_stats: None,
    }
}
```

Update `MeterStateSnapshot` accordingly.

- [ ] **Step 9: Add necessary imports**

Add `use readout_core::types::{TempSensorType, TempUnit, MathFunction, MathStats};` to driver.

- [ ] **Step 10: Build and verify**

Run: `cargo build -p readout-io`

- [ ] **Step 11: Commit**

```bash
git add crates/readout-io/src/multimeter_driver.rs
git commit -m "feat(io): add SCPI methods for dual display, null, math, filters, temp config"
```

---

## Chunk 2: Runtime Command Handling

### Task 3: Handle new commands in runtime

**Files:**
- Modify: `crates/readout-io/src/runtime.rs`

- [ ] **Step 1: Extend handle_meter_command**

Add new match arms in `handle_meter_command`:

```rust
MultimeterCommand::SetDualDisplay(enabled) => {
    if let Err(e) = driver.set_dual_display(enabled).await {
        tracing::warn!("set_dual_display failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
MultimeterCommand::SetNull(enabled) => {
    if let Err(e) = driver.set_null(enabled).await {
        tracing::warn!("set_null failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
MultimeterCommand::SetDcFilter(enabled) => {
    if let Err(e) = driver.set_dc_filter(enabled).await {
        tracing::warn!("set_dc_filter failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
MultimeterCommand::SetAutoImpedance(enabled) => {
    if let Err(e) = driver.set_auto_impedance(enabled).await {
        tracing::warn!("set_auto_impedance failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
MultimeterCommand::SetContinuityThreshold(ohms) => {
    if let Err(e) = driver.set_continuity_threshold(ohms).await {
        tracing::warn!("set_continuity_threshold failed: {e}");
    }
}
MultimeterCommand::SetTempSensorType(sensor) => {
    if let Err(e) = driver.set_temp_sensor_type(sensor).await {
        tracing::warn!("set_temp_sensor_type failed: {e}");
    }
}
MultimeterCommand::SetTempUnit(unit) => {
    if let Err(e) = driver.set_temp_unit(unit).await {
        tracing::warn!("set_temp_unit failed: {e}");
    }
}
MultimeterCommand::StartMath(func) => {
    if let Err(e) = driver.start_math(func).await {
        tracing::warn!("start_math failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
MultimeterCommand::StopMath => {
    if let Err(e) = driver.stop_math().await {
        tracing::warn!("stop_math failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
MultimeterCommand::QueryMathStats => {
    let stats = driver.query_math_stats().await;
    let state = driver.query_state().await;
    let _ = event_tx.send(RuntimeEvent::MeterState {
        identity: None,
        mode: state.mode,
        range_label: state.range_label,
        rate: state.rate,
        auto_range: state.auto_range,
        dual_display: state.dual_display,
        null_enabled: state.null_enabled,
        dc_filter: state.dc_filter,
        auto_impedance: state.auto_impedance,
        math_function: state.math_function,
        math_stats: stats,
    });
}
MultimeterCommand::ResetDevice => {
    if let Err(e) = driver.reset_device().await {
        tracing::warn!("reset_device failed: {e}");
    }
    emit_meter_state(driver, event_tx).await;
}
```

- [ ] **Step 2: Extract emit_meter_state helper**

Refactor the repeated pattern into a helper to avoid duplicating MeterState construction:

```rust
async fn emit_meter_state<T: ScpiTransport>(
    driver: &mut MultimeterDriver<T>,
    event_tx: &broadcast::Sender<RuntimeEvent>,
) {
    let state = driver.query_state().await;
    let _ = event_tx.send(RuntimeEvent::MeterState {
        identity: None,
        mode: state.mode,
        range_label: state.range_label,
        rate: state.rate,
        auto_range: state.auto_range,
        dual_display: state.dual_display,
        null_enabled: state.null_enabled,
        dc_filter: state.dc_filter,
        auto_impedance: state.auto_impedance,
        math_function: state.math_function,
        math_stats: state.math_stats,
    });
}
```

Update all existing match arms to use this helper too.

- [ ] **Step 3: Update existing MeterState emissions**

Update the initial MeterState emission after connect and all existing command handlers to include the new fields.

- [ ] **Step 4: Build and run tests**

Run: `cargo build -p readout-io && cargo test -p readout-io`

- [ ] **Step 5: Commit**

```bash
git add crates/readout-io/src/runtime.rs
git commit -m "feat(io): handle extended meter commands in runtime loop"
```

---

## Chunk 3: Meter Control UI

### Task 4: Add new sections to Meter Control panel

**Files:**
- Modify: `readout-gui/src/widgets/meter_control.rs`
- Modify: `readout-gui/src/app.rs` (window size)

- [ ] **Step 1: Add mode row 3 for missing modes**

Add Temperature and Period to mode selection:

```rust
let row3: &[(MeasurementMode, &str)] = &[
    (MeasurementMode::Temperature, "Temp"),
    (MeasurementMode::Period, "Per"),
];
```

Render like row1/row2.

- [ ] **Step 2: Add Dual Display toggle**

After Rate section, add:

```rust
ui.add_space(4.0);
ui.separator();

// Dual Display
ui.horizontal(|ui| {
    ui.label(egui::RichText::new("Dual Display").size(11.0).strong());
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut dual = state.meter_dual_display;
        if ui.checkbox(&mut dual, "Freq").changed() {
            send_command(command_tx, MultimeterCommand::SetDualDisplay(dual));
        }
    });
});
```

- [ ] **Step 3: Add NULL/REL toggle**

```rust
ui.horizontal(|ui| {
    ui.label(egui::RichText::new("Relative").size(11.0).strong());
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut null = state.meter_null_enabled;
        if ui.checkbox(&mut null, "NULL").changed() {
            send_command(command_tx, MultimeterCommand::SetNull(null));
        }
    });
});
```

- [ ] **Step 4: Add DC Voltage options (filter + auto impedance)**

Show only when mode is DcVoltage:

```rust
if state.meter_mode == MeasurementMode::DcVoltage {
    ui.add_space(4.0);
    ui.separator();
    ui.label(egui::RichText::new("DC Voltage").size(11.0).strong());
    ui.horizontal(|ui| {
        let mut filt = state.meter_dc_filter;
        if ui.checkbox(&mut filt, "DC Filter").changed() {
            send_command(command_tx, MultimeterCommand::SetDcFilter(filt));
        }
        let mut imp = state.meter_auto_impedance;
        if ui.checkbox(&mut imp, "Auto Z").changed() {
            send_command(command_tx, MultimeterCommand::SetAutoImpedance(imp));
        }
    });
}
```

- [ ] **Step 5: Add Temperature config**

Show only when mode is Temperature:

```rust
if state.meter_mode == MeasurementMode::Temperature {
    ui.add_space(4.0);
    ui.separator();
    ui.label(egui::RichText::new("Temperature").size(11.0).strong());
    ui.horizontal(|ui| {
        ui.label("Sensor:");
        if ui.selectable_label(false, "KITS90").clicked() {
            send_command(command_tx, MultimeterCommand::SetTempSensorType(TempSensorType::Kits90));
        }
        if ui.selectable_label(false, "PT100").clicked() {
            send_command(command_tx, MultimeterCommand::SetTempSensorType(TempSensorType::Pt100));
        }
    });
    ui.horizontal(|ui| {
        ui.label("Unit:");
        if ui.selectable_label(false, "°C").clicked() {
            send_command(command_tx, MultimeterCommand::SetTempUnit(TempUnit::Celsius));
        }
        if ui.selectable_label(false, "°F").clicked() {
            send_command(command_tx, MultimeterCommand::SetTempUnit(TempUnit::Fahrenheit));
        }
        if ui.selectable_label(false, "K").clicked() {
            send_command(command_tx, MultimeterCommand::SetTempUnit(TempUnit::Kelvin));
        }
    });
}
```

- [ ] **Step 6: Add Math/Statistics section**

```rust
ui.add_space(4.0);
ui.separator();
ui.label(egui::RichText::new("Math").size(11.0).strong());
ui.add_space(2.0);
ui.horizontal(|ui| {
    ui.spacing_mut().item_spacing.x = 3.0;
    let active = state.meter_math_function;
    if ui.selectable_label(active == Some(MathFunction::Average), "MIN/MAX").clicked() {
        if active == Some(MathFunction::Average) {
            send_command(command_tx, MultimeterCommand::StopMath);
        } else {
            send_command(command_tx, MultimeterCommand::StartMath(MathFunction::Average));
        }
    }
    if ui.selectable_label(active == Some(MathFunction::Null), "REL").clicked() {
        if active == Some(MathFunction::Null) {
            send_command(command_tx, MultimeterCommand::StopMath);
        } else {
            send_command(command_tx, MultimeterCommand::StartMath(MathFunction::Null));
        }
    }
});

if state.meter_math_function == Some(MathFunction::Average) {
    if let Some(stats) = &state.meter_math_stats {
        ui.horizontal(|ui| {
            let sec = theme::text_secondary(ui);
            ui.label(egui::RichText::new(format!("Min: {:.4}", stats.min)).size(10.0).color(sec));
            ui.label(egui::RichText::new(format!("Max: {:.4}", stats.max)).size(10.0).color(sec));
            ui.label(egui::RichText::new(format!("Avg: {:.4}", stats.avg)).size(10.0).color(sec));
        });
    }
    // Auto-refresh stats every frame when math is active
    send_command(command_tx, MultimeterCommand::QueryMathStats);
}
```

- [ ] **Step 7: Add Reset button**

At the bottom, before Beep section:

```rust
ui.add_space(4.0);
ui.separator();
if ui.button(egui::RichText::new(format!("{} Reset Device", icons::ARROW_COUNTER_CLOCKWISE)).size(11.0)).clicked() {
    send_command(command_tx, MultimeterCommand::ResetDevice);
}
```

- [ ] **Step 8: Increase Meter Control viewport size**

In `app.rs`, change meter control viewport inner_size from `[320.0, 280.0]` to `[320.0, 480.0]` to accommodate new sections.

- [ ] **Step 9: Add imports**

Add `use readout_core::types::{TempSensorType, TempUnit, MathFunction};` to meter_control.rs.

- [ ] **Step 10: Build and verify**

Run: `cargo build -p readout-gui`

- [ ] **Step 11: Commit**

```bash
git add readout-gui/src/widgets/meter_control.rs readout-gui/src/app.rs
git commit -m "feat(gui): add dual display, null, math, filter, temp controls to Meter Control"
```

---

## Chunk 4: Dual Display Measurement Reading

### Task 5: Read secondary measurement for dual display

**Files:**
- Modify: `crates/readout-io/src/multimeter_driver.rs`

- [ ] **Step 1: Query MEAS2 when dual display is active**

In `poll()`, after the primary measurement, add dual display query:

```rust
// After primary measurement parsing, before building DeviceMeasurement:
let (secondary_value, secondary_unit) = if self.dual_display_active {
    match self.transport.query("MEAS2?").await {
        Ok(Some(resp)) => {
            let parsed = MultimeterParser::parse(Some(&resp), "FREQ");
            match parsed {
                Some(p) => (p.value, Some(p.unit)),
                None => (None, None),
            }
        }
        _ => (None, None),
    }
} else {
    (None, None)
};
```

Set `secondary_value` and `secondary_unit` on the measurement.

- [ ] **Step 2: Track dual_display_active in driver state**

Add `dual_display_active: bool` field to `MultimeterDriver`. Update in `set_dual_display()`.

- [ ] **Step 3: Build and verify**

Run: `cargo build -p readout-io`

- [ ] **Step 4: Commit**

```bash
git add crates/readout-io/src/multimeter_driver.rs
git commit -m "feat(io): read secondary measurement when dual display is active"
```

---

## Chunk 5: Continuity Threshold from Config

### Task 6: Wire continuity threshold to config

**Files:**
- Modify: `crates/readout-io/src/multimeter_driver.rs`

- [ ] **Step 1: Send CONT:THRE on connect when in continuity mode**

In `connect()`, after beeper setup:

```rust
// Set continuity threshold if configured
if MeasurementModeParser::parse(Some(&self.current_mode)) == MeasurementMode::Continuity {
    if self.alert_config.short_threshold > 0.0 {
        let cmd = format!("CONT:THRE {}", self.alert_config.short_threshold);
        let _ = self.transport.query(&cmd).await;
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p readout-io`

- [ ] **Step 3: Commit**

```bash
git add crates/readout-io/src/multimeter_driver.rs
git commit -m "feat(io): send continuity threshold from config on connect"
```

---

## Chunk 6: Final Integration & Testing

### Task 7: Integration test and cleanup

- [ ] **Step 1: Build entire workspace**

Run: `cargo build`

- [ ] **Step 2: Run all tests**

Run: `cargo test`

- [ ] **Step 3: Test with simulator**

Run: `cargo run -p readout-gui -- --simulator` — verify Meter Control panel shows all new sections, buttons don't crash.

- [ ] **Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix: integration fixes for extended meter control"
```

- [ ] **Step 5: Push**

```bash
git push
```
