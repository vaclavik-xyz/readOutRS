use crate::transport::{ScpiTransport, TransportError};
use readout_core::alerts::{AlertConfiguration, AlertEvaluator};
use readout_core::measurement_mode::MeasurementModeParser;
use readout_core::multimeter_parser::MultimeterParser;
use readout_core::types::{DeviceId, DeviceMeasurement};
use std::time::Instant;

pub struct MultimeterDriver<T: ScpiTransport> {
    transport: T,
    current_mode: String,
    alert_config: AlertConfiguration,
    alarm_state: readout_core::types::AlarmState,
    meter_beep_enabled: bool,
}

impl<T: ScpiTransport> MultimeterDriver<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            current_mode: String::new(),
            alert_config: AlertConfiguration::default(),
            alarm_state: readout_core::types::AlarmState::None,
            meter_beep_enabled: false,
        }
    }

    pub fn set_alert_config(&mut self, config: AlertConfiguration) {
        self.alert_config = config;
    }

    pub fn set_meter_beep(&mut self, enabled: bool) {
        self.meter_beep_enabled = enabled;
    }

    pub async fn connect(&mut self) -> Result<(), TransportError> {
        self.transport.open().await?;

        // Verify the device actually responds — a serial port can be opened
        // even if the device on the other end is powered off.
        match self.transport.query("FUNC?").await {
            Ok(Some(mode)) => {
                self.current_mode = mode;
            }
            Ok(None) => {
                self.transport.close().await;
                return Err(TransportError::Timeout);
            }
            Err(e) => {
                self.transport.close().await;
                return Err(e);
            }
        }

        // Configure beeper based on config
        let beep_cmd = if self.meter_beep_enabled {
            "SYST:BEEP:STAT ON"
        } else {
            "SYST:BEEP:STAT OFF"
        };
        let _ = self.transport.query(beep_cmd).await;

        Ok(())
    }

    pub async fn poll(&mut self) -> Result<DeviceMeasurement, TransportError> {
        // Query current mode
        match self.transport.query("FUNC?").await {
            Ok(Some(mode)) => self.current_mode = mode.trim_matches('"').to_string(),
            Ok(None) => {} // keep previous mode
            Err(e) => tracing::warn!("FUNC? query failed, using previous mode: {e}"),
        }

        // Query measurement
        let response = self.transport.query("MEAS?").await?;

        let parsed = MultimeterParser::parse(response.as_deref(), &self.current_mode);

        let mode = MeasurementModeParser::parse(Some(&self.current_mode));

        let (primary_value, unit, is_overload, is_open) = match parsed {
            Some(p) => (p.value, p.unit, p.is_overload, p.is_open),
            None => (None, String::new(), false, false),
        };

        let mut measurement = DeviceMeasurement {
            timestamp: Instant::now(),
            device: DeviceId::Multimeter,
            primary_value,
            primary_unit: unit,
            secondary_value: None,
            secondary_unit: None,
            power_watts: None,
            energy_mwh: None,
            energy_mah: None,
            mode,
            mode_string: self.current_mode.clone(),
            is_overload,
            is_open,
            is_short: false,
            alarm_state: readout_core::types::AlarmState::None,
        };

        // Evaluate alarm state with hysteresis
        self.alarm_state = AlertEvaluator::evaluate(&mut measurement, &self.alert_config, self.alarm_state);
        measurement.alarm_state = self.alarm_state;

        Ok(measurement)
    }

    pub async fn set_beeper(&mut self, enabled: bool) {
        let cmd = if enabled { "SYST:BEEP:STAT ON" } else { "SYST:BEEP:STAT OFF" };
        let _ = self.transport.query(cmd).await;
        self.meter_beep_enabled = enabled;
    }

    pub async fn close(&mut self) {
        self.transport.close().await;
    }
}
