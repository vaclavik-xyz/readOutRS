use crate::chart_pipeline::ChartPipeline;
use crate::types::*;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const CHART_CAPACITY: usize = 360_000;
const LOG_BUFFER_SIZE: usize = 200;

/// Which USB-C metric to display on the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsbCMetric {
    Voltage,
    Current,
    Power,
    Energy,
}

pub const USBC_METRICS: &[(UsbCMetric, &str)] = &[
    (UsbCMetric::Voltage, "V"),
    (UsbCMetric::Current, "A"),
    (UsbCMetric::Power, "W"),
    (UsbCMetric::Energy, "mWh"),
];

pub struct DashboardState {
    pub latest_measurement: HashMap<DeviceId, DeviceMeasurement>,
    pub connection_state: HashMap<DeviceId, ConnectionState>,
    pub alarm_state: HashMap<DeviceId, AlarmState>,
    pub chart_pipelines: HashMap<DeviceId, ChartPipeline>,
    pub usbc_chart_pipelines: HashMap<UsbCMetric, ChartPipeline>,
    pub log_entries: VecDeque<LogEntry>,
    pub health: HealthMetrics,
    pub paused: bool,
    pub log_capture_enabled: bool,
    start_time: Instant,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct HealthMetrics {
    pub reconnect_count: u32,
    pub error_count: u32,
    pub parse_error_count: u32,
    pub output_drop_count: u32,
    pub measurement_count: u64,
}

impl DashboardState {
    pub fn new() -> Self {
        let mut chart_pipelines = HashMap::new();
        chart_pipelines.insert(DeviceId::Multimeter, ChartPipeline::new(CHART_CAPACITY));
        chart_pipelines.insert(DeviceId::UsbC, ChartPipeline::new(CHART_CAPACITY));

        let mut usbc_chart_pipelines = HashMap::new();
        usbc_chart_pipelines.insert(UsbCMetric::Voltage, ChartPipeline::new(CHART_CAPACITY));
        usbc_chart_pipelines.insert(UsbCMetric::Current, ChartPipeline::new(CHART_CAPACITY));
        usbc_chart_pipelines.insert(UsbCMetric::Power, ChartPipeline::new(CHART_CAPACITY));
        usbc_chart_pipelines.insert(UsbCMetric::Energy, ChartPipeline::new(CHART_CAPACITY));

        Self {
            latest_measurement: HashMap::new(),
            connection_state: HashMap::new(),
            alarm_state: HashMap::new(),
            chart_pipelines,
            usbc_chart_pipelines,
            log_entries: VecDeque::new(),
            health: HealthMetrics::default(),
            paused: false,
            log_capture_enabled: true,
            start_time: Instant::now(),
        }
    }

    pub fn handle_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Measurement { device, value } => {
                if !self.paused {
                    self.health.measurement_count += 1;
                    let elapsed = self.start_time.elapsed();
                    // Push to primary chart pipeline
                    if let Some(pipeline) = self.chart_pipelines.get_mut(&device) {
                        if let Some(v) = value.primary_value {
                            pipeline.push(elapsed, v);
                        }
                    }
                    // Push USB-C secondary metrics
                    if device == DeviceId::UsbC {
                        if let Some(v) = value.primary_value {
                            if let Some(p) = self.usbc_chart_pipelines.get_mut(&UsbCMetric::Voltage) {
                                p.push(elapsed, v);
                            }
                        }
                        if let Some(v) = value.secondary_value {
                            if let Some(p) = self.usbc_chart_pipelines.get_mut(&UsbCMetric::Current) {
                                p.push(elapsed, v);
                            }
                        }
                        if let Some(v) = value.power_watts {
                            if let Some(p) = self.usbc_chart_pipelines.get_mut(&UsbCMetric::Power) {
                                p.push(elapsed, v);
                            }
                        }
                        if let Some(v) = value.energy_mwh {
                            if let Some(p) = self.usbc_chart_pipelines.get_mut(&UsbCMetric::Energy) {
                                p.push(elapsed, v);
                            }
                        }
                    }
                    self.latest_measurement.insert(device, value);
                }
            }
            RuntimeEvent::AlarmTriggered { device, alarm } => {
                self.alarm_state.insert(device, alarm);
            }
            RuntimeEvent::AlarmCleared { device } => {
                self.alarm_state.insert(device, AlarmState::None);
            }
            RuntimeEvent::ConnectionChanged { device, state } => {
                if matches!(state, ConnectionState::Reconnecting) {
                    self.health.reconnect_count += 1;
                }
                // Clear stale measurement on disconnection
                if !matches!(state, ConnectionState::Connected) {
                    self.latest_measurement.remove(&device);
                    self.alarm_state.insert(device, AlarmState::None);
                }
                let label = match device {
                    DeviceId::Multimeter => "Multimeter",
                    DeviceId::UsbC => "USB-C",
                };
                let msg = match &state {
                    ConnectionState::Connected => format!("{label} connected"),
                    ConnectionState::Connecting => format!("{label} connecting..."),
                    ConnectionState::Reconnecting => format!("{label} reconnecting..."),
                    ConnectionState::Disconnected => format!("{label} disconnected"),
                    ConnectionState::Error(e) => format!("{label} error: {e}"),
                };
                let level = match &state {
                    ConnectionState::Connected => LogLevel::Info,
                    ConnectionState::Error(_) => LogLevel::Error,
                    _ => LogLevel::Warning,
                };
                self.push_log(level, msg);
                self.connection_state.insert(device, state);
            }
            RuntimeEvent::Error { message, .. } => {
                self.health.error_count += 1;
                self.push_log(LogLevel::Error, message);
            }
            RuntimeEvent::Log { level, message } => {
                self.push_log(level, message);
            }
        }
    }

    fn push_log(&mut self, level: LogLevel, message: String) {
        if !self.log_capture_enabled {
            return;
        }
        self.log_entries.push_back(LogEntry { level, message });
        if self.log_entries.len() > LOG_BUFFER_SIZE {
            self.log_entries.pop_front();
        }
    }

    pub fn connection_for(&self, device: DeviceId) -> &ConnectionState {
        self.connection_state
            .get(&device)
            .unwrap_or(&ConnectionState::Disconnected)
    }

    pub fn alarm_for(&self, device: DeviceId) -> AlarmState {
        self.alarm_state
            .get(&device)
            .copied()
            .unwrap_or(AlarmState::None)
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}
