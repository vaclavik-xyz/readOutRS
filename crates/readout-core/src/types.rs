use std::time::Instant;

use crate::measurement_mode::MeasurementMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DeviceId {
    Multimeter,
    UsbC,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlarmState {
    None,
    HighAlarm,
    LowAlarm,
    Open,
    Short,
}

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
    Null,
    Average,
    Db,
    Dbm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DbReference {
    Ohms(u16), // 50, 75, 93, 110, 124, 125, 135, 150, 250, 300, 500, 600, 800, 900, 1000, 1200, 8000
}

pub const DB_REFERENCE_VALUES: &[u16] = &[
    50, 75, 93, 110, 124, 125, 135, 150, 250, 300, 500, 600, 800, 900, 1000, 1200, 8000,
];

#[derive(Debug, Clone, Copy)]
pub struct MathStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub enum MultimeterCommand {
    QueryIdentity,
    SetMode(crate::measurement_mode::MeasurementMode),
    SetRange(MultimeterRange),
    SetRate(MultimeterRate),
    SetDualDisplay(bool),
    SetNull(bool),
    SetDcFilter(bool),
    SetAutoImpedance(bool),
    SetContinuityThreshold(f64),
    SetTempSensorType(TempSensorType),
    SetTempUnit(TempUnit),
    StartMath(MathFunction),
    StopMath,
    QueryMathStats,
    SetDbReference(DbReference),
    SetRemoteMode(bool), // SYST:REM / SYST:LOC
    ResetDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct DeviceMeasurement {
    pub timestamp: Instant,
    pub device: DeviceId,
    pub primary_value: Option<f64>,
    pub primary_unit: String,
    pub secondary_value: Option<f64>,
    pub secondary_unit: Option<String>,
    pub power_watts: Option<f64>,
    pub energy_mwh: Option<f64>,
    pub energy_mah: Option<f64>,
    pub mode: MeasurementMode,
    pub mode_string: String,
    pub is_overload: bool,
    pub is_open: bool,
    pub is_short: bool,
    pub alarm_state: AlarmState,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    Measurement {
        device: DeviceId,
        value: DeviceMeasurement,
    },
    AlarmTriggered {
        device: DeviceId,
        alarm: AlarmState,
    },
    AlarmCleared {
        device: DeviceId,
    },
    ConnectionChanged {
        device: DeviceId,
        state: ConnectionState,
    },
    Error {
        device: DeviceId,
        message: String,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    MeterState {
        identity: Option<String>,
        mode: crate::measurement_mode::MeasurementMode,
        range_label: String,
        rate: MultimeterRate,
        auto_range: bool,
        dual_display: bool,
        null_enabled: bool,
        dc_filter: bool,
        auto_impedance: bool,
        math_function: Option<MathFunction>,
        math_stats: Option<MathStats>,
    },
}

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
