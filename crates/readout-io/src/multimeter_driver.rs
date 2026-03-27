use crate::transport::{ScpiTransport, TransportError};
use readout_core::alerts::{AlertConfiguration, AlertEvaluator};
use readout_core::measurement_mode::{MeasurementMode, MeasurementModeParser};
use readout_core::multimeter_parser::MultimeterParser;
use readout_core::types::{DeviceId, DeviceMeasurement, MultimeterRange, MultimeterRate};
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

    pub async fn query_identity(&mut self) -> Option<String> {
        match self.transport.query("*IDN?").await {
            Ok(Some(s)) => Some(s.trim().to_string()),
            _ => None,
        }
    }

    pub async fn set_mode(&mut self, mode: MeasurementMode) -> Result<(), TransportError> {
        let cmd = mode_to_scpi(mode).ok_or(TransportError::Timeout)?;
        let _ = self.transport.query(cmd).await?;
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
        let mode = match self.transport.query("FUNC?").await {
            Ok(Some(m)) => {
                let clean = m.trim_matches('"').to_string();
                self.current_mode = clean.clone();
                MeasurementModeParser::parse(Some(&clean))
            }
            _ => MeasurementModeParser::parse(Some(&self.current_mode)),
        };
        let range_label = self.transport.query("RANGE?").await
            .ok().flatten()
            .unwrap_or_default()
            .trim().to_string();
        let auto_range = self.transport.query("AUTO?").await
            .ok().flatten()
            .map(|s| s.trim() == "1")
            .unwrap_or(true);
        let rate = self.transport.query("RATE?").await
            .ok().flatten()
            .map(|s| parse_rate(&s))
            .unwrap_or(MultimeterRate::Medium);
        MeterStateSnapshot { mode, range_label, rate, auto_range }
    }
}

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
