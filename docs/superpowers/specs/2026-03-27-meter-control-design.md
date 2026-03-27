# Multimeter Control — SCPI Infrastructure & GUI

**Date:** 2026-03-27
**Scope:** Etapy 1+2 — command infrastructure + mode/range/rate control + Meter Control window
**Device:** OWON XDM1241 (= XDM1041, shared SCPI implementation)
**Approach:** Dedicated command channel + driver methods (approach A)

---

## Overview

Add remote control of the OWON XDM1241 multimeter from a dedicated native OS window. The main dashboard remains a passive read-only overlay. A new "Multimeter Control" window (separate OS viewport) lets the user switch measurement mode, range, rate, and see device identity.

Data flow: GUI → `Command::Meter(MultimeterCommand)` → Runtime command loop → `mm_cmd_tx` channel → multimeter task → `MultimeterDriver` SCPI methods → device. State flows back via `RuntimeEvent::MeterState` → `DashboardState` → GUI.

---

## 1. Types (`readout-core/src/types.rs`)

### New enums

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

#[derive(Debug, Clone)]
pub enum MultimeterCommand {
    QueryIdentity,                // *IDN?
    SetMode(MeasurementMode),     // CONF:VOLT:DC, CONF:RES, ...
    SetRange(MultimeterRange),    // RANGE <n> or AUTO
    SetRate(MultimeterRate),      // RATE {F|M|S}
}
```

### Extend `Command`

```rust
pub enum Command {
    Start,
    Stop,
    Rescan,
    ResetEnergy { device: DeviceId },
    AcknowledgeAlarm { device: DeviceId },
    SilenceAlarm { duration: std::time::Duration },
    Meter(MultimeterCommand), // NEW — forwarded to meter task
}
```

### New `RuntimeEvent` variant

```rust
RuntimeEvent::MeterState {
    identity: Option<String>,   // *IDN? response, None if not yet queried
    mode: MeasurementMode,      // current mode from FUNC?
    range_label: String,        // RANGE? response, e.g. "500 mV", "50 kΩ"
    rate: MultimeterRate,       // RATE? response
    auto_range: bool,           // AUTO? response
}
```

`range_label` is the raw string from `RANGE?` — avoids needing a per-mode range enum. GUI displays it as-is.

---

## 2. MeasurementMode mapping

`MeasurementMode` already covers the modes the XDM1241 supports. One fix needed: `FRES` currently maps to `Resistance` — add `FourWireResistance` variant (or keep as-is if UI doesn't distinguish). For this spec: keep as-is, both map to Resistance.

Mode → SCPI command mapping (in driver):

| MeasurementMode | SCPI Command |
|---|---|
| DcVoltage | `CONF:VOLT:DC` |
| AcVoltage | `CONF:VOLT:AC` |
| DcCurrent | `CONF:CURR:DC` |
| AcCurrent | `CONF:CURR:AC` |
| Resistance | `CONF:RES` |
| Capacitance | `CONF:CAP` |
| Frequency | `CONF:FREQ` |
| Diode | `CONF:DIOD` |
| Continuity | `CONF:CONT` |
| Temperature | `CONF:TEMP` |
| Period | `CONF:PER` |

`Unknown` mode is not sendable — GUI doesn't offer it.

---

## 3. MultimeterDriver — new methods

```rust
impl<T: ScpiTransport> MultimeterDriver<T> {
    // --- Existing ---
    // connect(), poll(), set_beeper(), close()

    // --- New ---

    /// Query *IDN? and return raw response.
    pub async fn query_identity(&mut self) -> Option<String>;

    /// Send CONF:<mode>, then query_state() to confirm.
    pub async fn set_mode(&mut self, mode: MeasurementMode) -> Result<(), TransportError>;

    /// Send AUTO or RANGE <n>, then query RANGE? + AUTO?.
    pub async fn set_range(&mut self, range: MultimeterRange) -> Result<(), TransportError>;

    /// Send RATE {F|M|S}.
    pub async fn set_rate(&mut self, rate: MultimeterRate) -> Result<(), TransportError>;

    /// Query FUNC? + RANGE? + AUTO? + RATE? and return snapshot.
    pub async fn query_state(&mut self) -> MeterStateSnapshot;
}

pub struct MeterStateSnapshot {
    pub mode: MeasurementMode,
    pub range_label: String,
    pub rate: MultimeterRate,
    pub auto_range: bool,
}
```

### `connect()` changes

After successful connection:
1. Query `*IDN?` and store identity
2. Call `query_state()` for initial state
3. Emit `RuntimeEvent::MeterState` with identity + state

### SCPI response parsing

- `FUNC?` → strip quotes, existing `MeasurementModeParser::parse()`
- `RANGE?` → raw string, e.g. `"500 mV"`, `"50 kOHM"` — stored as-is
- `AUTO?` → `"1"` = true, `"0"` = false
- `RATE?` → `"F"` / `"M"` / `"S"` → `MultimeterRate`
- `*IDN?` → raw string, e.g. `"OWON,XDM1241,1546011,V1.0.0,3"`

### XDM1241 quirk: `FUNC?` vs `FUNC1?`

Current code uses `FUNC?`. The XDM1241 manual documents `FUNC1?`. If `FUNC?` works on real hardware (which the existing code implies), keep it. If not, switch to `FUNC1?`. This is a runtime-verified compatibility point, not a design decision — driver should try `FUNC?` first and fall back to `FUNC1?` on timeout/empty response during connect.

### XDM1241 quirk: non-query responses

Non-query commands (`CONF:VOLT:DC`, `AUTO`, `RANGE 3`, `RATE F`) return `OK\nOK\nOK\n` regardless of validity. Driver must ignore the response content for non-query commands and verify state change via follow-up queries.

---

## 4. Runtime — meter command channel

### Channel setup in `run()`

```rust
let (mm_cmd_tx, mm_cmd_rx) = mpsc::channel::<MultimeterCommand>(16);
// Pass mm_cmd_rx to run_multimeter() → multimeter_loop()
```

### Command loop extension

```rust
Some(Command::Meter(cmd)) => {
    let _ = mm_cmd_tx.send(cmd).await;
}
```

### `multimeter_loop` — command processing

Same pattern as USB-C after user's refactor: drain pending commands via `try_recv` before each blocking `poll()`.

```rust
while let Ok(cmd) = cmd_rx.try_recv() {
    Self::handle_meter_command(driver, &event_tx, cmd).await;
}
// then normal poll()
```

`handle_meter_command`:
- Calls the appropriate driver method
- On success, calls `driver.query_state()` and emits `RuntimeEvent::MeterState`
- For `QueryIdentity`, also queries and includes identity

### Initial state emission

After `driver.connect()` succeeds, emit `MeterState` with identity and initial state. This gives the GUI the starting values before any user interaction.

---

## 5. Simulator extension

`SimulatedScpiTransport` gains internal state:

```rust
struct SimulatedScpiTransport {
    // existing: sample_rate_hz, is_open, sample_index, beeper_enabled
    current_mode: String,     // NEW — default "VOLT:DC"
    auto_range: bool,         // NEW — default true
    range_index: u8,          // NEW — default 3
    rate: char,               // NEW — default 'M'
}
```

New command handling:

| Command | Simulator behavior |
|---|---|
| `*IDN?` | Returns `"SIMULATED,READOUT,MULTIMETER,1.0"` (already works) |
| `CONF:VOLT:DC` etc. | Sets `current_mode`, resets `auto_range=true`, `range_index` to default |
| `AUTO` | Sets `auto_range=true` |
| `AUTO?` | Returns `"1"` or `"0"` |
| `RANGE <n>` | Sets `range_index=n`, `auto_range=false` |
| `RANGE?` | Returns simulated label based on mode + range_index |
| `RATE F/M/S` | Sets `rate` |
| `RATE?` | Returns `rate` as string |
| `FUNC?` | Returns `current_mode` (existing, already mode-aware) |

Value generation uses `current_mode` (already does this via `current_mode()` method — just change it from cycle-based to state-based).

---

## 6. DashboardState

New fields:

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

`handle_event` for `MeterState`:
- Update all meter_* fields
- `meter_identity` is only set when `identity.is_some()` (first connect or explicit query)

On disconnect: meter_* fields are NOT cleared — they represent last known device state. Connection LED already indicates disconnection.

---

## 7. GUI — Meter Control window

### File: `readout-gui/src/widgets/meter_control.rs`

```rust
pub struct MeterControlPanel {
    pub open: bool,
}
```

No draft/save cycle — all actions send commands immediately.

### Opening

Separate native OS window via egui viewport API:

```rust
// In app.rs update():
if self.meter_control.open {
    let state = &self.state;
    let command_tx = self.runtime.as_ref().map(|r| r.command_tx.clone());
    let connected = matches!(
        state.connection_for(DeviceId::Multimeter),
        ConnectionState::Connected
    );

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("meter_control"),
        egui::ViewportBuilder::default()
            .with_title("Multimeter Control")
            .with_inner_size([320.0, 280.0])
            .with_resizable(false),
        |ctx, _class| {
            meter_control::show(ctx, state, command_tx.as_ref(), connected);
        },
    );
}
```

### Toolbar button

New `ToolbarAction::OpenMeterControl`. Visible only when `show_mm == true`. Icon: "⚙" or similar small button.

### Window layout

```
┌─ Multimeter Control ────────────────┐
│ OWON XDM1241 · V1.0.0              │  ← parsed from *IDN?
│─────────────────────────────────────│
│ Mode                                │
│ [V DC] [V AC] [A DC] [A AC]        │  ← row 1
│ [Ω] [Cap] [Hz] [Diod] [Cont]      │  ← row 2
│─────────────────────────────────────│
│ Range              [☑ Auto]         │
│ ◀ 500 mV ▶                         │  ← step buttons, disabled when auto
│─────────────────────────────────────│
│ Rate                                │
│ [Fast] [Medium] [Slow]             │
└─────────────────────────────────────┘
```

- Active mode/rate highlighted via `selectable_value`
- Range step buttons send `Manual(current ± 1)`, clamped to 1-7
- Entire window content disabled (greyed out) when multimeter not connected
- Identity row hidden until `meter_identity.is_some()`
- Window closable via OS close button → sets `meter_control.open = false`

### Command sending

```rust
fn send_command(command_tx: &tokio::sync::mpsc::Sender<Command>, cmd: MultimeterCommand) {
    let _ = command_tx.try_send(Command::Meter(cmd));
}
```

Non-blocking `try_send` — if channel is full, command is dropped (acceptable for UI-triggered actions).

---

## 8. Testing

### Unit tests

- `MultimeterDriver` with simulated transport: verify `set_mode()` sends correct SCPI, `query_state()` returns expected values
- `MeterStateSnapshot` parsing from simulated responses
- `MultimeterCommand` → SCPI string mapping

### Integration test

- Extend existing `runtime_simulator_produces_measurements_and_shuts_down` to also send `Command::Meter(SetMode(...))` and verify `MeterState` event is emitted with correct mode

### Manual verification

- `FUNC?` vs `FUNC1?` compatibility on real XDM1241 hardware
- Response timing for CONF:* commands (may need short delay before follow-up queries)

---

## 9. Out of scope

- Dual display (`FUNC2`, `MEAS2?`) — etapa 3
- CALC:AVER:* hardware statistics — etapa 3
- NULL/relative measurement — etapa 4
- DC filter, auto impedance, CONT:THRE, TEMP:RTD:* — etapa 4
- dB/dBm, SYST:REM/LOC, *RST — not planned for GUI
- FRES as separate mode in GUI — keep merged with Resistance for now
