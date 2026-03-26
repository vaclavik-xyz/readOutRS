use crate::chart_pipeline::ChartPipeline;
use crate::types::*;
use std::collections::HashMap;
use std::time::Duration;

const CHART_CAPACITY: usize = 360_000;
const LOG_BUFFER_SIZE: usize = 200;

pub struct DashboardState {
    pub latest_measurement: HashMap<DeviceId, DeviceMeasurement>,
    pub connection_state: HashMap<DeviceId, ConnectionState>,
    pub alarm_state: HashMap<DeviceId, AlarmState>,
    pub chart_pipelines: HashMap<DeviceId, ChartPipeline>,
    pub log_entries: Vec<LogEntry>,
    pub health: HealthMetrics,
    pub paused: bool,
    elapsed: Duration,
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

        Self {
            latest_measurement: HashMap::new(),
            connection_state: HashMap::new(),
            alarm_state: HashMap::new(),
            chart_pipelines,
            log_entries: Vec::new(),
            health: HealthMetrics::default(),
            paused: false,
            elapsed: Duration::ZERO,
        }
    }

    pub fn handle_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Measurement { device, value } => {
                if !self.paused {
                    self.health.measurement_count += 1;
                    // Push to chart pipeline
                    if let Some(pipeline) = self.chart_pipelines.get_mut(&device) {
                        if let Some(v) = value.primary_value {
                            self.elapsed += Duration::from_millis(1); // monotonic approx
                            pipeline.push(self.elapsed, v);
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
        self.log_entries.push(LogEntry { level, message });
        if self.log_entries.len() > LOG_BUFFER_SIZE {
            self.log_entries.remove(0);
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
