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
}

#[derive(Debug, Clone)]
pub enum Command {
    Start,
    Stop,
    Rescan,
    ResetEnergy { device: DeviceId },
    AcknowledgeAlarm { device: DeviceId },
    SilenceAlarm { duration: std::time::Duration },
}
