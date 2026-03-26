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
}

impl<T: ScpiTransport> MultimeterDriver<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            current_mode: String::new(),
            alert_config: AlertConfiguration::default(),
        }
    }

    pub fn set_alert_config(&mut self, config: AlertConfiguration) {
        self.alert_config = config;
    }

    pub async fn connect(&mut self) -> Result<(), TransportError> {
        self.transport.open().await?;

        // Query initial mode
        if let Ok(Some(mode)) = self.transport.query("FUNC?").await {
            self.current_mode = mode;
        }

        // Configure beeper
        let _ = self.transport.query("SYST:BEEP:STAT ON").await;

        Ok(())
    }

    pub async fn poll(&mut self) -> Result<DeviceMeasurement, TransportError> {
        // Query current mode
        if let Ok(Some(mode)) = self.transport.query("FUNC?").await {
            self.current_mode = mode;
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
        };

        // Enrich with alert info
        let _ = AlertEvaluator::evaluate(&mut measurement, &self.alert_config, readout_core::types::AlarmState::None);

        Ok(measurement)
    }

    pub async fn close(&mut self) {
        self.transport.close().await;
    }
}
