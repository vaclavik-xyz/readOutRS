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

#[derive(Debug, Clone)]
pub enum MultimeterCommand {
    QueryIdentity,
    SetMode(crate::measurement_mode::MeasurementMode),
    SetRange(MultimeterRange),
    SetRate(MultimeterRate),
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
