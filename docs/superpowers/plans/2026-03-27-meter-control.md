# Multimeter Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add SCPI remote control of the OWON XDM1241 multimeter from a dedicated native OS window — mode, range, rate, identity.

**Architecture:** New `MultimeterCommand` enum flows GUI → Runtime → Driver via dedicated mpsc channel (same pattern as USB-C reset). Driver executes SCPI commands and emits `RuntimeEvent::MeterState` back. Meter Control lives in a separate OS viewport.

**Tech Stack:** Rust, egui 0.33 (multi-viewport), tokio mpsc, eframe

**Spec:** `docs/superpowers/specs/2026-03-27-meter-control-design.md`

---

### Task 1: Core types

**Files:**
- Modify: `crates/readout-core/src/types.rs`

- [ ] **Step 1: Add MultimeterRange, MultimeterRate, MultimeterCommand enums**

Add after the existing `AlarmState` enum and before `LogLevel`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultimeterRange {
    Auto,
    Manual(u8), // index 1-7, meaning depends on current mode
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultimeterRate {
    Fast,   // RATE F
    Medium, // RATE M
    Slow,   // RATE S
}
```

- [ ] **Step 2: Add MultimeterCommand enum**

Add after `MultimeterRate`:

```rust
#[derive(Debug, Clone)]
pub enum MultimeterCommand {
    QueryIdentity,
    SetMode(crate::measurement_mode::MeasurementMode),
    SetRange(MultimeterRange),
    SetRate(MultimeterRate),
}
```

- [ ] **Step 3: Extend Command enum**

Add new variant to the existing `Command` enum:

```rust
#[derive(Debug, Clone)]
pub enum Command {
    Start,
    Stop,
    Rescan,
    ResetEnergy { device: DeviceId },
    AcknowledgeAlarm { device: DeviceId },
    SilenceAlarm { duration: std::time::Duration },
    Meter(MultimeterCommand),
}
```

- [ ] **Step 4: Add MeterState variant to RuntimeEvent**

Add to the existing `RuntimeEvent` enum:

```rust
    MeterState {
        identity: Option<String>,
        mode: crate::measurement_mode::MeasurementMode,
        range_label: String,
        rate: MultimeterRate,
        auto_range: bool,
    },
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p readout-core 2>&1 | head -5`
Expected: successful build (warnings OK)

- [ ] **Step 6: Commit**

```bash
git add crates/readout-core/src/types.rs
git commit -m "feat: add MultimeterCommand, MultimeterRange, MultimeterRate types and MeterState event"
```

---

### Task 2: Simulator SCPI extension

**Files:**
- Modify: `crates/readout-io/src/simulated.rs`
- Modify: `crates/readout-io/tests/simulated_test.rs`

- [ ] **Step 1: Write failing tests for new SCPI commands**

Add to `crates/readout-io/tests/simulated_test.rs`:

```rust
#[tokio::test]
async fn scpi_conf_changes_mode() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    // Default mode
    let mode = t.query("FUNC?").await.unwrap().unwrap();
    assert_eq!(mode, "VOLT:DC");

    // Switch to resistance
    let _ = t.query("CONF:RES").await;
    let mode = t.query("FUNC?").await.unwrap().unwrap();
    assert_eq!(mode, "RES");

    // Switch to AC voltage
    let _ = t.query("CONF:VOLT:AC").await;
    let mode = t.query("FUNC?").await.unwrap().unwrap();
    assert_eq!(mode, "VOLT:AC");
}

#[tokio::test]
async fn scpi_auto_range() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    // Default is auto
    let auto = t.query("AUTO?").await.unwrap().unwrap();
    assert_eq!(auto, "1");

    // Set manual range
    let _ = t.query("RANGE 2").await;
    let auto = t.query("AUTO?").await.unwrap().unwrap();
    assert_eq!(auto, "0");

    // Back to auto
    let _ = t.query("AUTO").await;
    let auto = t.query("AUTO?").await.unwrap().unwrap();
    assert_eq!(auto, "1");
}

#[tokio::test]
async fn scpi_range_query() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    let _ = t.query("RANGE 3").await;
    let range = t.query("RANGE?").await.unwrap().unwrap();
    // Simulator returns a label for the current mode + index
    assert!(!range.is_empty());
}

#[tokio::test]
async fn scpi_rate_control() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    let rate = t.query("RATE?").await.unwrap().unwrap();
    assert_eq!(rate, "M"); // default medium

    let _ = t.query("RATE F").await;
    let rate = t.query("RATE?").await.unwrap().unwrap();
    assert_eq!(rate, "F");

    let _ = t.query("RATE S").await;
    let rate = t.query("RATE?").await.unwrap().unwrap();
    assert_eq!(rate, "S");
}

#[tokio::test]
async fn scpi_conf_resets_range_to_auto() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    // Set manual range
    let _ = t.query("RANGE 4").await;
    let auto = t.query("AUTO?").await.unwrap().unwrap();
    assert_eq!(auto, "0");

    // CONF:* resets to auto
    let _ = t.query("CONF:CURR:DC").await;
    let auto = t.query("AUTO?").await.unwrap().unwrap();
    assert_eq!(auto, "1");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-io --test simulated_test 2>&1 | tail -20`
Expected: 5 new tests FAIL

- [ ] **Step 3: Implement simulator state and command handling**

Replace the entire `SimulatedScpiTransport` implementation in `crates/readout-io/src/simulated.rs`. The struct gains state fields, `current_mode()` becomes a getter for the state field, mode cycling is removed, and `query()` handles new commands:

```rust
const DEFAULT_MODE: &str = "VOLT:DC";

pub struct SimulatedScpiTransport {
    sample_rate_hz: u32,
    is_open: bool,
    sample_index: usize,
    beeper_enabled: bool,
    mode: String,
    auto_range: bool,
    range_index: u8,
    rate: char,
}

impl SimulatedScpiTransport {
    pub fn new(sample_rate_hz: u32) -> Self {
        assert!(sample_rate_hz > 0, "sample_rate_hz must be > 0");
        Self {
            sample_rate_hz,
            is_open: false,
            sample_index: 0,
            beeper_enabled: false,
            mode: DEFAULT_MODE.into(),
            auto_range: true,
            range_index: 3,
            rate: 'M',
        }
    }

    fn generate_value(&self) -> String {
        let t = self.sample_index as f64 / self.sample_rate_hz as f64;
        match self.mode.as_str() {
            "VOLT:DC" => format!("{:.6E}", 12.0 + 0.25 * (t * 1.2).sin()),
            "CURR:DC" => format!("{:.6E}", 1.4 + 0.3 * (t * 1.8 + 0.5).sin()),
            "RES" => {
                if self.sample_index % 160 == 0 && self.sample_index > 0 {
                    return "OL".into();
                }
                format!("{:.6E}", 120.0 + 45.0 * (t * 0.8).sin())
            }
            "VOLT:AC" => format!("{:.6E}", 230.0 + 8.0 * (t * 0.6).sin()),
            "CURR:AC" => format!("{:.6E}", 0.5 + 0.15 * (t * 1.0).sin()),
            "CAP" => format!("{:.6E}", 4.7e-6 + 0.2e-6 * (t * 0.5).sin()),
            "FREQ" => format!("{:.6E}", 50.0 + 0.1 * (t * 0.3).sin()),
            "PER" => format!("{:.6E}", 0.02 + 0.0001 * (t * 0.3).sin()),
            "CONT" => format!("{:.6E}", 0.5 + 0.3 * (t * 2.0).sin()),
            "DIOD" => format!("{:.6E}", 0.65 + 0.02 * (t * 0.8).sin()),
            "TEMP" => format!("{:.6E}", 23.0 + 1.5 * (t * 0.2).sin()),
            _ => "0.0E+00".into(),
        }
    }

    fn range_label(&self) -> String {
        let labels: &[&str] = match self.mode.as_str() {
            "VOLT:DC" => &["50 mV", "500 mV", "5 V", "50 V", "500 V", "1000 V"],
            "VOLT:AC" => &["500 mV", "5 V", "50 V", "500 V", "750 V"],
            "CURR:DC" | "CURR:AC" => &["500 uA", "5 mA", "50 mA", "500 mA", "5 A", "10 A"],
            "RES" | "FRES" => &["500 OHM", "5 kOHM", "50 kOHM", "500 kOHM", "5 MOHM", "50 MOHM"],
            "CAP" => &["50 nF", "500 nF", "5 uF", "50 uF", "500 uF", "5 mF", "50 mF"],
            _ => &["---"],
        };
        let idx = (self.range_index as usize).saturating_sub(1).min(labels.len() - 1);
        labels[idx].into()
    }

    fn handle_conf(&mut self, cmd: &str) {
        let new_mode = if cmd.contains("VOLT:DC") || cmd == "CONF:VOLT" {
            "VOLT:DC"
        } else if cmd.contains("VOLT:AC") {
            "VOLT:AC"
        } else if cmd.contains("CURR:DC") || cmd == "CONF:CURR" {
            "CURR:DC"
        } else if cmd.contains("CURR:AC") {
            "CURR:AC"
        } else if cmd.contains("RES") {
            "RES"
        } else if cmd.contains("FRES") {
            "FRES"
        } else if cmd.contains("CAP") {
            "CAP"
        } else if cmd.contains("FREQ") {
            "FREQ"
        } else if cmd.contains("PER") {
            "PER"
        } else if cmd.contains("DIOD") {
            "DIOD"
        } else if cmd.contains("CONT") {
            "CONT"
        } else if cmd.contains("TEMP") {
            "TEMP"
        } else {
            return;
        };
        self.mode = new_mode.into();
        self.auto_range = true;
        self.range_index = 3;
    }
}
```

Update the `ScpiTransport` impl `query()` method to handle new commands:

```rust
impl ScpiTransport for SimulatedScpiTransport {
    async fn query(&mut self, command: &str) -> Result<Option<String>, TransportError> {
        if !self.is_open {
            return Err(TransportError::NotOpen);
        }

        let cmd = command.trim().to_uppercase();

        if cmd == "*IDN?" {
            return Ok(Some("SIMULATED,READOUT,MULTIMETER,1.0".into()));
        }

        if cmd == "FUNC?" || cmd == "FUNC1?" {
            return Ok(Some(self.mode.clone()));
        }

        if cmd.starts_with("MEAS") {
            let value = self.generate_value();
            self.sample_index += 1;
            tokio::time::sleep(std::time::Duration::from_micros(
                1_000_000 / self.sample_rate_hz as u64,
            ))
            .await;
            return Ok(Some(value));
        }

        if cmd == "SYST:BEEP:STAT?" {
            return Ok(Some(if self.beeper_enabled { "1" } else { "0" }.into()));
        }

        if cmd.starts_with("SYST:BEEP:STAT") {
            self.beeper_enabled = cmd.ends_with(" ON");
            return Ok(None);
        }

        if cmd.starts_with("CONF:") {
            self.handle_conf(&cmd);
            return Ok(None);
        }

        if cmd == "AUTO" {
            self.auto_range = true;
            return Ok(None);
        }

        if cmd == "AUTO?" {
            return Ok(Some(if self.auto_range { "1" } else { "0" }.into()));
        }

        if cmd.starts_with("RANGE ") {
            if let Some(n) = cmd.strip_prefix("RANGE ").and_then(|s| s.parse::<u8>().ok()) {
                self.range_index = n.clamp(1, 7);
                self.auto_range = false;
            }
            return Ok(None);
        }

        if cmd == "RANGE?" {
            return Ok(Some(self.range_label()));
        }

        if cmd.starts_with("RATE ") {
            if let Some(c) = cmd.strip_prefix("RATE ").and_then(|s| s.chars().next()) {
                if matches!(c, 'F' | 'M' | 'S') {
                    self.rate = c;
                }
            }
            return Ok(None);
        }

        if cmd == "RATE?" {
            return Ok(Some(self.rate.to_string()));
        }

        Ok(None)
    }
}
```

- [ ] **Step 4: Update existing tests that relied on mode cycling**

Replace `scpi_mode_cycling` test:

```rust
#[tokio::test]
async fn scpi_mode_cycling() {
    let mut t = SimulatedScpiTransport::new(10);
    t.open().await.unwrap();

    // Mode stays fixed until CONF changes it
    for _ in 0..300 {
        let _ = t.query("MEAS?").await;
    }
    let mode = t.query("FUNC?").await.unwrap().unwrap();
    assert_eq!(mode, "VOLT:DC"); // still default, no cycling

    // Switch mode via CONF
    let _ = t.query("CONF:CURR:DC").await;
    let mode = t.query("FUNC?").await.unwrap().unwrap();
    assert_eq!(mode, "CURR:DC");
}
```

- [ ] **Step 5: Run all simulator tests**

Run: `cargo test -p readout-io --test simulated_test 2>&1 | tail -25`
Expected: all tests PASS

- [ ] **Step 6: Update driver mode_changes test**

In `crates/readout-io/tests/driver_test.rs`, replace `multimeter_driver_mode_changes`:

```rust
#[tokio::test]
async fn multimeter_driver_mode_changes() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    // Mode starts as DcVoltage
    let m = driver.poll().await.unwrap();
    assert_eq!(m.mode, MeasurementMode::DcVoltage);
}
```

- [ ] **Step 7: Run driver tests**

Run: `cargo test -p readout-io --test driver_test 2>&1 | tail -15`
Expected: all tests PASS

- [ ] **Step 8: Commit**

```bash
git add crates/readout-io/src/simulated.rs crates/readout-io/tests/simulated_test.rs crates/readout-io/tests/driver_test.rs
git commit -m "feat: extend simulator with CONF, AUTO, RANGE, RATE SCPI commands"
```

---

### Task 3: Driver methods

**Files:**
- Modify: `crates/readout-io/src/multimeter_driver.rs`
- Modify: `crates/readout-io/tests/driver_test.rs`

- [ ] **Step 1: Write failing tests for new driver methods**

Add to `crates/readout-io/tests/driver_test.rs`:

```rust
use readout_core::types::{MultimeterRange, MultimeterRate};

#[tokio::test]
async fn multimeter_driver_query_identity() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    let identity = driver.query_identity().await;
    assert!(identity.is_some());
    assert!(identity.unwrap().contains("SIMULATED"));
}

#[tokio::test]
async fn multimeter_driver_set_mode() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    driver.set_mode(MeasurementMode::Resistance).await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.mode, MeasurementMode::Resistance);
    assert!(state.auto_range); // CONF resets to auto
}

#[tokio::test]
async fn multimeter_driver_set_range() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    driver.set_range(MultimeterRange::Manual(4)).await.unwrap();
    let state = driver.query_state().await;
    assert!(!state.auto_range);
    assert!(!state.range_label.is_empty());

    driver.set_range(MultimeterRange::Auto).await.unwrap();
    let state = driver.query_state().await;
    assert!(state.auto_range);
}

#[tokio::test]
async fn multimeter_driver_set_rate() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    driver.set_rate(MultimeterRate::Fast).await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.rate, MultimeterRate::Fast);

    driver.set_rate(MultimeterRate::Slow).await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.rate, MultimeterRate::Slow);
}

#[tokio::test]
async fn multimeter_driver_query_state() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    let state = driver.query_state().await;
    assert_eq!(state.mode, MeasurementMode::DcVoltage);
    assert_eq!(state.rate, MultimeterRate::Medium);
    assert!(state.auto_range);
    assert!(!state.range_label.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-io --test driver_test 2>&1 | tail -20`
Expected: 5 new tests FAIL (methods don't exist yet)

- [ ] **Step 3: Implement MeterStateSnapshot and driver methods**

Add to `crates/readout-io/src/multimeter_driver.rs`:

```rust
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{MultimeterRange, MultimeterRate};
```

Add `MeterStateSnapshot` struct and helper after the `impl` block:

```rust
pub struct MeterStateSnapshot {
    pub mode: MeasurementMode,
    pub range_label: String,
    pub rate: MultimeterRate,
    pub auto_range: bool,
}

fn mode_to_scpi(mode: MeasurementMode) -> Option<&'static str> {
    match mode {
        MeasurementMode::DcVoltage => Some("CONF:VOLT:DC"),
        MeasurementMode::AcVoltage => Some("CONF:VOLT:AC"),
        MeasurementMode::DcCurrent => Some("CONF:CURR:DC"),
        MeasurementMode::AcCurrent => Some("CONF:CURR:AC"),
        MeasurementMode::Resistance => Some("CONF:RES"),
        MeasurementMode::Capacitance => Some("CONF:CAP"),
        MeasurementMode::Frequency => Some("CONF:FREQ"),
        MeasurementMode::Diode => Some("CONF:DIOD"),
        MeasurementMode::Continuity => Some("CONF:CONT"),
        MeasurementMode::Temperature => Some("CONF:TEMP"),
        MeasurementMode::Period => Some("CONF:PER"),
        MeasurementMode::Unknown => None,
    }
}

fn parse_rate(s: &str) -> MultimeterRate {
    match s.trim() {
        "F" => MultimeterRate::Fast,
        "S" => MultimeterRate::Slow,
        _ => MultimeterRate::Medium,
    }
}
```

Add new methods inside the existing `impl<T: ScpiTransport> MultimeterDriver<T>` block:

```rust
    pub async fn query_identity(&mut self) -> Option<String> {
        match self.transport.query("*IDN?").await {
            Ok(Some(s)) => Some(s.trim().to_string()),
            _ => None,
        }
    }

    pub async fn set_mode(&mut self, mode: MeasurementMode) -> Result<(), TransportError> {
        let cmd = mode_to_scpi(mode).ok_or(TransportError::Timeout)?;
        let _ = self.transport.query(cmd).await?;
        // Re-query to confirm mode change
        if let Ok(Some(m)) = self.transport.query("FUNC?").await {
            self.current_mode = m.trim_matches('"').to_string();
        }
        Ok(())
    }

    pub async fn set_range(&mut self, range: MultimeterRange) -> Result<(), TransportError> {
        match range {
            MultimeterRange::Auto => {
                let _ = self.transport.query("AUTO").await?;
            }
            MultimeterRange::Manual(n) => {
                let cmd = format!("RANGE {}", n.clamp(1, 7));
                let _ = self.transport.query(&cmd).await?;
            }
        }
        Ok(())
    }

    pub async fn set_rate(&mut self, rate: MultimeterRate) -> Result<(), TransportError> {
        let cmd = match rate {
            MultimeterRate::Fast => "RATE F",
            MultimeterRate::Medium => "RATE M",
            MultimeterRate::Slow => "RATE S",
        };
        let _ = self.transport.query(cmd).await?;
        Ok(())
    }

    pub async fn query_state(&mut self) -> MeterStateSnapshot {
        // FUNC?
        let mode = match self.transport.query("FUNC?").await {
            Ok(Some(m)) => {
                let clean = m.trim_matches('"').to_string();
                self.current_mode = clean.clone();
                MeasurementModeParser::parse(Some(&clean))
            }
            _ => MeasurementModeParser::parse(Some(&self.current_mode)),
        };

        // RANGE?
        let range_label = self.transport.query("RANGE?").await
            .ok().flatten()
            .unwrap_or_default()
            .trim().to_string();

        // AUTO?
        let auto_range = self.transport.query("AUTO?").await
            .ok().flatten()
            .map(|s| s.trim() == "1")
            .unwrap_or(true);

        // RATE?
        let rate = self.transport.query("RATE?").await
            .ok().flatten()
            .map(|s| parse_rate(&s))
            .unwrap_or(MultimeterRate::Medium);

        MeterStateSnapshot { mode, range_label, rate, auto_range }
    }
```

- [ ] **Step 4: Run driver tests**

Run: `cargo test -p readout-io --test driver_test 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/readout-io/src/multimeter_driver.rs crates/readout-io/tests/driver_test.rs
git commit -m "feat: add query_identity, set_mode, set_range, set_rate, query_state to MultimeterDriver"
```

---

### Task 4: Runtime meter command channel

**Files:**
- Modify: `crates/readout-io/src/runtime.rs`
- Modify: `crates/readout-io/tests/runtime_test.rs`

- [ ] **Step 1: Write failing integration test**

Add to `crates/readout-io/tests/runtime_test.rs`:

```rust
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{MultimeterCommand, MultimeterRate};

#[tokio::test]
async fn runtime_meter_command_emits_state() {
    let config = readout_persistence::config::AppConfiguration {
        use_simulator: true,
        multimeter_enabled: true,
        usbc_enabled: false,
        ..Default::default()
    };

    let (runtime, mut event_rx) = readout_io::runtime::Runtime::new(config);
    let command_tx = runtime.command_sender();
    let cancel = tokio_util::sync::CancellationToken::new();
    let cancel_clone = cancel.clone();

    let handle = tokio::spawn(async move {
        runtime.run(cancel_clone).await;
    });

    // Wait for initial MeterState after connect
    let mut got_initial_state = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(200), event_rx.recv()).await {
            Ok(Ok(readout_core::types::RuntimeEvent::MeterState { mode, .. })) => {
                assert_eq!(mode, MeasurementMode::DcVoltage);
                got_initial_state = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => continue,
        }
    }
    assert!(got_initial_state, "should receive initial MeterState after connect");

    // Send mode change command
    command_tx.send(readout_core::types::Command::Meter(
        MultimeterCommand::SetMode(MeasurementMode::Resistance),
    )).await.unwrap();

    // Wait for MeterState with new mode
    let mut got_mode_change = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(200), event_rx.recv()).await {
            Ok(Ok(readout_core::types::RuntimeEvent::MeterState { mode, .. })) => {
                if mode == MeasurementMode::Resistance {
                    got_mode_change = true;
                    break;
                }
            }
            Ok(Ok(_)) => continue,
            _ => continue,
        }
    }
    assert!(got_mode_change, "should receive MeterState with Resistance after SetMode");

    cancel.cancel();
    let _ = handle.await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p readout-io --test runtime_test runtime_meter_command 2>&1 | tail -10`
Expected: FAIL (Command::Meter variant doesn't match in runtime, no MeterState emitted)

- [ ] **Step 3: Add mm_cmd channel and forwarding in Runtime::run()**

In `crates/readout-io/src/runtime.rs`, add the meter command channel setup alongside the existing USB-C one. In `run()`, add:

```rust
// Channel for forwarding commands to the multimeter device task
let (mm_cmd_tx, mm_cmd_rx) = mpsc::channel::<readout_core::types::MultimeterCommand>(16);
```

Pass `mm_cmd_rx` to `run_multimeter()`:

Update `run_multimeter` signature to accept `cmd_rx: mpsc::Receiver<readout_core::types::MultimeterCommand>`, and pass it through to `multimeter_loop`.

Update `multimeter_loop` signature to accept `cmd_rx: &mut mpsc::Receiver<readout_core::types::MultimeterCommand>`.

In the command loop, add a match arm:

```rust
Some(Command::Meter(cmd)) => {
    let _ = mm_cmd_tx.send(cmd).await;
}
```

- [ ] **Step 4: Restructure multimeter_loop for command draining**

Replace the inner `select!` loop in `multimeter_loop` with the same pattern as USB-C — check cancel, drain commands, then poll:

```rust
// After successful connect, before inner loop:
// Emit initial meter state
let identity = driver.query_identity().await;
let initial_state = driver.query_state().await;
let _ = event_tx.send(RuntimeEvent::MeterState {
    identity,
    mode: initial_state.mode,
    range_label: initial_state.range_label,
    rate: initial_state.rate,
    auto_range: initial_state.auto_range,
});

let mut consecutive_errors: u32 = 0;
loop {
    if cancel.is_cancelled() {
        driver.close().await;
        let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
            device: DeviceId::Multimeter,
            state: readout_core::types::ConnectionState::Disconnected,
        });
        return;
    }

    // Drain pending meter commands
    while let Ok(cmd) = cmd_rx.try_recv() {
        Self::handle_meter_command(driver, &event_tx, cmd).await;
    }

    // Live meter beep toggle
    let desired = meter_beep_flag.load(Ordering::Relaxed);
    if desired != current_beep_state {
        driver.set_beeper(desired).await;
        current_beep_state = desired;
    }

    match driver.poll().await {
        Ok(measurement) => {
            consecutive_errors = 0;
            let new_alarm = measurement.alarm_state;
            let _ = event_tx.send(RuntimeEvent::Measurement {
                device: DeviceId::Multimeter,
                value: measurement,
            });
            if new_alarm != prev_alarm {
                if new_alarm == readout_core::types::AlarmState::None {
                    let _ = event_tx.send(RuntimeEvent::AlarmCleared {
                        device: DeviceId::Multimeter,
                    });
                } else {
                    let _ = event_tx.send(RuntimeEvent::AlarmTriggered {
                        device: DeviceId::Multimeter,
                        alarm: new_alarm,
                    });
                }
                prev_alarm = new_alarm;
            }
        }
        Err(e) => {
            consecutive_errors += 1;
            let _ = event_tx.send(RuntimeEvent::Error {
                device: DeviceId::Multimeter,
                message: e.to_string(),
            });
            if consecutive_errors >= 5 {
                tracing::warn!("Multimeter: too many consecutive errors, will reconnect");
                driver.close().await;
                prev_alarm = readout_core::types::AlarmState::None;
                break;
            }
            tokio::select! {
                _ = cancel.cancelled() => {
                    driver.close().await;
                    let _ = event_tx.send(RuntimeEvent::ConnectionChanged {
                        device: DeviceId::Multimeter,
                        state: readout_core::types::ConnectionState::Disconnected,
                    });
                    return;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
            }
        }
    }
}
```

- [ ] **Step 5: Add handle_meter_command helper**

Add as a static method on `Runtime`:

```rust
async fn handle_meter_command<T: ScpiTransport>(
    driver: &mut MultimeterDriver<T>,
    event_tx: &broadcast::Sender<RuntimeEvent>,
    cmd: readout_core::types::MultimeterCommand,
) {
    use readout_core::types::MultimeterCommand;
    match cmd {
        MultimeterCommand::QueryIdentity => {
            let identity = driver.query_identity().await;
            let state = driver.query_state().await;
            let _ = event_tx.send(RuntimeEvent::MeterState {
                identity,
                mode: state.mode,
                range_label: state.range_label,
                rate: state.rate,
                auto_range: state.auto_range,
            });
        }
        MultimeterCommand::SetMode(mode) => {
            if let Err(e) = driver.set_mode(mode).await {
                tracing::warn!("set_mode failed: {e}");
            }
            let state = driver.query_state().await;
            let _ = event_tx.send(RuntimeEvent::MeterState {
                identity: None,
                mode: state.mode,
                range_label: state.range_label,
                rate: state.rate,
                auto_range: state.auto_range,
            });
        }
        MultimeterCommand::SetRange(range) => {
            if let Err(e) = driver.set_range(range).await {
                tracing::warn!("set_range failed: {e}");
            }
            let state = driver.query_state().await;
            let _ = event_tx.send(RuntimeEvent::MeterState {
                identity: None,
                mode: state.mode,
                range_label: state.range_label,
                rate: state.rate,
                auto_range: state.auto_range,
            });
        }
        MultimeterCommand::SetRate(rate) => {
            if let Err(e) = driver.set_rate(rate).await {
                tracing::warn!("set_rate failed: {e}");
            }
            let state = driver.query_state().await;
            let _ = event_tx.send(RuntimeEvent::MeterState {
                identity: None,
                mode: state.mode,
                range_label: state.range_label,
                rate: state.rate,
                auto_range: state.auto_range,
            });
        }
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p readout-io 2>&1 | tail -30`
Expected: all tests PASS including the new `runtime_meter_command_emits_state`

- [ ] **Step 7: Commit**

```bash
git add crates/readout-io/src/runtime.rs crates/readout-io/tests/runtime_test.rs
git commit -m "feat: add meter command channel to runtime, emit MeterState on connect and command"
```

---

### Task 5: DashboardState meter fields

**Files:**
- Modify: `crates/readout-core/src/dashboard_state.rs`

- [ ] **Step 1: Add meter state fields**

Add imports at the top:

```rust
use crate::measurement_mode::MeasurementMode;
use crate::types::MultimeterRate;
```

Add fields to `DashboardState`:

```rust
pub struct DashboardState {
    // ... existing fields ...
    pub meter_identity: Option<String>,
    pub meter_mode: MeasurementMode,
    pub meter_range_label: String,
    pub meter_rate: MultimeterRate,
    pub meter_auto_range: bool,
}
```

Initialize in `new()`:

```rust
meter_identity: None,
meter_mode: MeasurementMode::Unknown,
meter_range_label: String::new(),
meter_rate: MultimeterRate::Medium,
meter_auto_range: true,
```

- [ ] **Step 2: Handle MeterState event**

Add to the `handle_event` match in `DashboardState`:

```rust
RuntimeEvent::MeterState { identity, mode, range_label, rate, auto_range } => {
    if let Some(id) = identity {
        self.meter_identity = Some(id);
    }
    self.meter_mode = mode;
    self.meter_range_label = range_label;
    self.meter_rate = rate;
    self.meter_auto_range = auto_range;
}
```

- [ ] **Step 3: Build and test**

Run: `cargo test 2>&1 | tail -10`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/readout-core/src/dashboard_state.rs
git commit -m "feat: add meter state fields to DashboardState"
```

---

### Task 6: Toolbar button

**Files:**
- Modify: `readout-gui/src/widgets/toolbar.rs`

- [ ] **Step 1: Add OpenMeterControl to ToolbarAction and ToolbarState**

Add to `ToolbarAction` enum:

```rust
OpenMeterControl,
```

Add to `ToolbarState`:

```rust
pub show_mm: bool,  // already exists — needed for visibility check
```

- [ ] **Step 2: Add button to toolbar row 2**

In the `show()` function, add a meter control button in row 2, after the settings button, before the pin button. Only show when `state.show_mm` is true:

```rust
if state.show_mm {
    if ui
        .button(egui::RichText::new("🎛").size(10.0))
        .on_hover_text("Multimeter Control")
        .clicked()
    {
        action = ToolbarAction::OpenMeterControl;
    }
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p readout-gui 2>&1 | head -5`
Expected: successful build

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/widgets/toolbar.rs
git commit -m "feat: add Multimeter Control button to toolbar"
```

---

### Task 7: Meter Control window widget

**Files:**
- Create: `readout-gui/src/widgets/meter_control.rs`
- Modify: `readout-gui/src/widgets/mod.rs`

- [ ] **Step 1: Create meter_control.rs**

Create `readout-gui/src/widgets/meter_control.rs`:

```rust
use crate::theme;
use readout_core::dashboard_state::DashboardState;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{Command, ConnectionState, DeviceId, MultimeterCommand, MultimeterRange, MultimeterRate};

pub struct MeterControlPanel {
    pub open: bool,
}

impl MeterControlPanel {
    pub fn new() -> Self {
        Self { open: false }
    }
}

/// Render the Meter Control panel content.
/// `command_tx`: None if runtime is not running.
/// `connected`: whether the multimeter is currently connected.
pub fn show(
    ctx: &egui::Context,
    state: &DashboardState,
    command_tx: Option<&tokio::sync::mpsc::Sender<Command>>,
    connected: bool,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.set_enabled(connected && command_tx.is_some());

        // Identity
        if let Some(ref identity) = state.meter_identity {
            let parts: Vec<&str> = identity.split(',').collect();
            let label = if parts.len() >= 4 {
                format!("{} {} · {}", parts[0].trim(), parts[1].trim(), parts[3].trim())
            } else {
                identity.clone()
            };
            ui.label(
                egui::RichText::new(label)
                    .size(11.0)
                    .color(theme::text_secondary(ui)),
            );
            ui.separator();
        }

        // Mode section
        ui.label(egui::RichText::new("Mode").size(11.0).strong());
        ui.add_space(2.0);

        let current_mode = state.meter_mode;

        let row1: &[(MeasurementMode, &str)] = &[
            (MeasurementMode::DcVoltage, "V DC"),
            (MeasurementMode::AcVoltage, "V AC"),
            (MeasurementMode::DcCurrent, "A DC"),
            (MeasurementMode::AcCurrent, "A AC"),
        ];
        let row2: &[(MeasurementMode, &str)] = &[
            (MeasurementMode::Resistance, "Ω"),
            (MeasurementMode::Capacitance, "Cap"),
            (MeasurementMode::Frequency, "Hz"),
            (MeasurementMode::Diode, "Diod"),
            (MeasurementMode::Continuity, "Cont"),
        ];

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (mode, label) in row1 {
                if ui.selectable_label(current_mode == *mode, egui::RichText::new(*label).size(11.0)).clicked() {
                    send_command(command_tx, MultimeterCommand::SetMode(*mode));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (mode, label) in row2 {
                if ui.selectable_label(current_mode == *mode, egui::RichText::new(*label).size(11.0)).clicked() {
                    send_command(command_tx, MultimeterCommand::SetMode(*mode));
                }
            }
        });

        ui.add_space(4.0);
        ui.separator();

        // Range section
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Range").size(11.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut auto = state.meter_auto_range;
                if ui.checkbox(&mut auto, "Auto").changed() {
                    let range = if auto { MultimeterRange::Auto } else { MultimeterRange::Manual(3) };
                    send_command(command_tx, MultimeterCommand::SetRange(range));
                }
            });
        });

        ui.set_enabled(connected && command_tx.is_some() && !state.meter_auto_range);
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("◀").size(14.0)).clicked() {
                // Step range down (we don't know current index, send manual with low guess)
                send_command(command_tx, MultimeterCommand::SetRange(MultimeterRange::Manual(1)));
            }
            ui.label(
                egui::RichText::new(if state.meter_range_label.is_empty() { "---" } else { &state.meter_range_label })
                    .size(14.0)
                    .family(egui::FontFamily::Monospace),
            );
            if ui.button(egui::RichText::new("▶").size(14.0)).clicked() {
                send_command(command_tx, MultimeterCommand::SetRange(MultimeterRange::Manual(7)));
            }
        });
        ui.set_enabled(connected && command_tx.is_some());

        ui.add_space(4.0);
        ui.separator();

        // Rate section
        ui.label(egui::RichText::new("Rate").size(11.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let current_rate = state.meter_rate;
            for (rate, label) in &[(MultimeterRate::Fast, "Fast"), (MultimeterRate::Medium, "Medium"), (MultimeterRate::Slow, "Slow")] {
                if ui.selectable_label(current_rate == *rate, egui::RichText::new(*label).size(11.0)).clicked() {
                    send_command(command_tx, MultimeterCommand::SetRate(*rate));
                }
            }
        });
    });
}

fn send_command(command_tx: Option<&tokio::sync::mpsc::Sender<Command>>, cmd: MultimeterCommand) {
    if let Some(tx) = command_tx {
        let _ = tx.try_send(Command::Meter(cmd));
    }
}
```

- [ ] **Step 2: Register module**

Add to `readout-gui/src/widgets/mod.rs`:

```rust
pub mod meter_control;
```

- [ ] **Step 3: Build**

Run: `cargo build -p readout-gui 2>&1 | head -5`
Expected: successful build

- [ ] **Step 4: Commit**

```bash
git add readout-gui/src/widgets/meter_control.rs readout-gui/src/widgets/mod.rs
git commit -m "feat: add Meter Control window widget"
```

---

### Task 8: App integration

**Files:**
- Modify: `readout-gui/src/app.rs`

- [ ] **Step 1: Add MeterControlPanel to ReadOutApp**

Add field to the struct:

```rust
meter_control: widgets::meter_control::MeterControlPanel,
```

Initialize in `new()`:

```rust
meter_control: widgets::meter_control::MeterControlPanel::new(),
```

- [ ] **Step 2: Handle toolbar action**

In the toolbar action match, add:

```rust
widgets::toolbar::ToolbarAction::OpenMeterControl => {
    self.meter_control.open = true;
}
```

- [ ] **Step 3: Render viewport**

In `update()`, after the settings panel handling and before the main content, add the viewport rendering:

```rust
// Meter Control viewport
if self.meter_control.open {
    let connected = matches!(
        self.state.connection_for(DeviceId::Multimeter),
        ConnectionState::Connected
    );
    let command_tx = self.runtime.as_ref().map(|r| r.command_tx.clone());

    let mut close_requested = false;
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("meter_control"),
        egui::ViewportBuilder::default()
            .with_title("Multimeter Control")
            .with_inner_size([320.0, 280.0])
            .with_resizable(false),
        |ctx, _class| {
            close_requested = ctx.input(|i| i.viewport().close_requested());
            crate::theme::apply_theme(ctx, self.config.dashboard_theme);
            widgets::meter_control::show(ctx, &self.state, command_tx.as_ref(), connected);
        },
    );
    if close_requested {
        self.meter_control.open = false;
    }
}
```

- [ ] **Step 4: Add keyboard shortcut**

In the keyboard shortcuts section, add:

```rust
if i.modifiers.command && i.key_pressed(egui::Key::M) {
    self.meter_control.open = !self.meter_control.open;
}
```

- [ ] **Step 5: Build and run all tests**

Run: `cargo build 2>&1 | head -5`
Expected: successful build

Run: `cargo test 2>&1 | tail -10`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/app.rs
git commit -m "feat: wire Meter Control viewport to app with Cmd+M shortcut"
```
