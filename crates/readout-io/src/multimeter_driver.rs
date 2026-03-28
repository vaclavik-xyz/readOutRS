use crate::transport::{ScpiTransport, TransportError};
use readout_core::alerts::{AlertConfiguration, AlertEvaluator};
use readout_core::measurement_mode::{MeasurementMode, MeasurementModeParser};
use readout_core::multimeter_parser::MultimeterParser;
use readout_core::types::{
    DbReference, DeviceId, DeviceMeasurement, MathFunction, MathStats, MultimeterRange,
    MultimeterRate, TempSensorType, TempUnit,
};
use std::time::Instant;

pub struct MultimeterDriver<T: ScpiTransport> {
    transport: T,
    current_mode: String,
    alert_config: AlertConfiguration,
    alarm_state: readout_core::types::AlarmState,
    meter_beep_enabled: bool,
    dual_display_active: bool,
    dual_display_mode: String,
}

impl<T: ScpiTransport> MultimeterDriver<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            current_mode: String::new(),
            alert_config: AlertConfiguration::default(),
            alarm_state: readout_core::types::AlarmState::None,
            meter_beep_enabled: false,
            dual_display_active: false,
            dual_display_mode: "FREQ".into(),
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

        // Set continuity threshold if configured
        if MeasurementModeParser::parse(Some(&self.current_mode)) == MeasurementMode::Continuity
            && self.alert_config.short_threshold > 0.0
        {
            let cmd = format!("CONT:THRE {}", self.alert_config.short_threshold);
            let _ = self.transport.query(&cmd).await?;
        }

        // Configure beeper based on config
        let beep_cmd = if self.meter_beep_enabled {
            "SYST:BEEP:STAT ON"
        } else {
            "SYST:BEEP:STAT OFF"
        };
        let _ = self.transport.query(beep_cmd).await;

        // Sync dual display state from device
        self.dual_display_active = self.query_dual_display().await;

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

        // Query secondary measurement when dual display is active
        let (secondary_value, secondary_unit) = if self.dual_display_active {
            match self.transport.query("MEAS2?").await {
                Ok(Some(resp)) => {
                    match MultimeterParser::parse(Some(&resp), &self.dual_display_mode) {
                        Some(p) => (p.value, Some(p.unit)),
                        None => (None, None),
                    }
                }
                _ => (None, None),
            }
        } else {
            (None, None)
        };

        let mut measurement = DeviceMeasurement {
            timestamp: Instant::now(),
            device: DeviceId::Multimeter,
            primary_value,
            primary_unit: unit,
            secondary_value,
            secondary_unit,
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
        self.alarm_state =
            AlertEvaluator::evaluate(&mut measurement, &self.alert_config, self.alarm_state);
        measurement.alarm_state = self.alarm_state;

        Ok(measurement)
    }

    pub async fn set_beeper(&mut self, enabled: bool) {
        let cmd = if enabled {
            "SYST:BEEP:STAT ON"
        } else {
            "SYST:BEEP:STAT OFF"
        };
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
        let cmd = mode_to_scpi(mode)
            .ok_or_else(|| TransportError::ConnectionLost("unsupported measurement mode".into()))?;
        let _ = self.transport.query(cmd).await?;
        match self.transport.query("FUNC?").await {
            Ok(Some(m)) => self.current_mode = m.trim_matches('"').to_string(),
            Ok(None) => tracing::warn!("FUNC? readback after set_mode returned empty"),
            Err(e) => tracing::warn!("FUNC? readback after set_mode failed: {e}"),
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

    pub async fn set_dual_display(&mut self, enabled: bool) -> Result<(), TransportError> {
        let cmd = if enabled {
            "FUNC2 \"FREQuency\""
        } else {
            "FUNC2 \"NONe\""
        };
        let _ = self.transport.query(cmd).await?;
        self.dual_display_active = enabled;
        Ok(())
    }

    pub async fn query_dual_display(&mut self) -> bool {
        match self.transport.query("FUNC2?").await.ok().flatten() {
            Some(s) => {
                let trimmed = s.trim().trim_matches('"').to_uppercase();
                if trimmed.contains("NON") {
                    false
                } else {
                    self.dual_display_mode = trimmed;
                    true
                }
            }
            None => false,
        }
    }

    pub async fn set_null(&mut self, enabled: bool) -> Result<(), TransportError> {
        if let Some(p) = self.sense_prefix() {
            let cmd = format!("{p}:NULL {}", if enabled { "ON" } else { "OFF" });
            let _ = self.transport.query(&cmd).await?;
        }
        Ok(())
    }

    pub async fn query_null(&mut self) -> bool {
        if let Some(p) = self.sense_prefix() {
            let cmd = format!("{p}:NULL?");
            return self
                .transport
                .query(&cmd)
                .await
                .ok()
                .flatten()
                .map(|s| s.trim() == "1" || s.trim().to_uppercase() == "ON")
                .unwrap_or(false);
        }
        false
    }

    pub async fn set_dc_filter(&mut self, enabled: bool) -> Result<(), TransportError> {
        let cmd = format!("VOLT:DC:FILT {}", if enabled { "ON" } else { "OFF" });
        let _ = self.transport.query(&cmd).await?;
        Ok(())
    }

    pub async fn set_auto_impedance(&mut self, enabled: bool) -> Result<(), TransportError> {
        let cmd = format!("VOLT:DC:IMP:AUTO {}", if enabled { "ON" } else { "OFF" });
        let _ = self.transport.query(&cmd).await?;
        Ok(())
    }

    pub async fn set_continuity_threshold(&mut self, ohms: f64) -> Result<(), TransportError> {
        let cmd = format!("CONT:THRE {}", ohms);
        let _ = self.transport.query(&cmd).await?;
        Ok(())
    }

    pub async fn set_temp_sensor_type(
        &mut self,
        sensor: TempSensorType,
    ) -> Result<(), TransportError> {
        let cmd = match sensor {
            TempSensorType::Kits90 => "TEMP:RTD:TYPE KITS90",
            TempSensorType::Pt100 => "TEMP:RTD:TYPE PT100",
        };
        let _ = self.transport.query(cmd).await?;
        Ok(())
    }

    pub async fn set_temp_unit(&mut self, unit: TempUnit) -> Result<(), TransportError> {
        let cmd = match unit {
            TempUnit::Celsius => "TEMP:RTD:UNIT C",
            TempUnit::Fahrenheit => "TEMP:RTD:UNIT F",
            TempUnit::Kelvin => "TEMP:RTD:UNIT K",
        };
        let _ = self.transport.query(cmd).await?;
        Ok(())
    }

    pub async fn start_math(&mut self, func: MathFunction) -> Result<(), TransportError> {
        let cmd = match func {
            MathFunction::Null => "CALC:FUNC NULL",
            MathFunction::Average => "CALC:FUNC AVERage",
            MathFunction::Db => "CALC:FUNC DB",
            MathFunction::Dbm => "CALC:FUNC DBM",
        };
        let _ = self.transport.query(cmd).await?;
        let _ = self.transport.query("CALC:STAT ON").await?;
        Ok(())
    }

    pub async fn stop_math(&mut self) -> Result<(), TransportError> {
        let _ = self.transport.query("CALC:STAT OFF").await?;
        Ok(())
    }

    pub async fn query_math_stats(&mut self) -> Option<MathStats> {
        let response = self.transport.query("CALC:AVER:ALL?").await.ok()??;
        let parts: Vec<&str> = response.trim().split(',').collect();
        if parts.len() >= 4 {
            Some(MathStats {
                min: parts[0].trim().parse().ok()?,
                max: parts[1].trim().parse().ok()?,
                avg: parts[2].trim().parse().ok()?,
                count: parts[3].trim().parse().ok()?,
            })
        } else {
            None
        }
    }

    pub async fn set_db_reference(&mut self, reference: DbReference) -> Result<(), TransportError> {
        let DbReference::Ohms(ohms) = reference;
        let _ = self
            .transport
            .query(&format!("CALC:DB:REF {}", ohms))
            .await?;
        let _ = self
            .transport
            .query(&format!("CALC:DBM:REF {}", ohms))
            .await?;
        Ok(())
    }

    pub async fn set_remote_mode(&mut self, remote: bool) -> Result<(), TransportError> {
        let cmd = if remote { "SYST:REM" } else { "SYST:LOC" };
        let _ = self.transport.query(cmd).await?;
        Ok(())
    }

    pub async fn reset_device(&mut self) -> Result<(), TransportError> {
        let _ = self.transport.query("*RST").await?;
        self.dual_display_active = false;
        self.dual_display_mode = "FREQ".into();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    fn sense_prefix(&self) -> Option<&'static str> {
        let mode = MeasurementModeParser::parse(Some(&self.current_mode));
        match mode {
            MeasurementMode::DcVoltage => Some("VOLT:DC"),
            MeasurementMode::AcVoltage => Some("VOLT:AC"),
            MeasurementMode::DcCurrent => Some("CURR:DC"),
            MeasurementMode::AcCurrent => Some("CURR:AC"),
            MeasurementMode::Resistance => Some("RES"),
            MeasurementMode::Capacitance => Some("CAP"),
            MeasurementMode::Temperature => Some("TEMP:RTD"),
            _ => None,
        }
    }

    pub async fn query_state(&mut self) -> MeterStateSnapshot {
        let mode = match self.transport.query("FUNC?").await {
            Ok(Some(m)) => {
                self.current_mode = m.trim_matches('"').to_string();
                MeasurementModeParser::parse(Some(&self.current_mode))
            }
            _ => MeasurementModeParser::parse(Some(&self.current_mode)),
        };
        let range_label = self
            .transport
            .query("RANGE?")
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
            .trim()
            .to_string();
        let auto_range = self
            .transport
            .query("AUTO?")
            .await
            .ok()
            .flatten()
            .map(|s| s.trim() == "1")
            .unwrap_or(true);
        let rate = self
            .transport
            .query("RATE?")
            .await
            .ok()
            .flatten()
            .map(|s| parse_rate(&s))
            .unwrap_or(MultimeterRate::Medium);
        let dual_display = self.query_dual_display().await;
        self.dual_display_active = dual_display;
        let null_enabled = self.query_null().await;

        MeterStateSnapshot {
            mode,
            range_label,
            rate,
            auto_range,
            dual_display,
            null_enabled,
            dc_filter: false,
            auto_impedance: false,
            math_function: None,
            math_stats: None,
        }
    }
}

pub struct MeterStateSnapshot {
    pub mode: MeasurementMode,
    pub range_label: String,
    pub rate: MultimeterRate,
    pub auto_range: bool,
    pub dual_display: bool,
    pub null_enabled: bool,
    pub dc_filter: bool,
    pub auto_impedance: bool,
    pub math_function: Option<MathFunction>,
    pub math_stats: Option<MathStats>,
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
