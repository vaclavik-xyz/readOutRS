use crate::measurement_mode::MeasurementMode;
use crate::types::{AlarmState, DeviceMeasurement};

const HYSTERESIS_FRACTION: f64 = 0.01;

#[derive(Debug, Clone, PartialEq)]
pub struct AlertConfiguration {
    pub short_threshold: f64,
    pub dcv_high_alarm_enabled: bool,
    pub dcv_high_alarm_value: f64,
    pub dcv_low_alarm_enabled: bool,
    pub dcv_low_alarm_value: f64,
}

impl Default for AlertConfiguration {
    fn default() -> Self {
        Self {
            short_threshold: 2.0,
            dcv_high_alarm_enabled: false,
            dcv_high_alarm_value: 12.0,
            dcv_low_alarm_enabled: false,
            dcv_low_alarm_value: 0.0,
        }
    }
}

pub struct AlertEvaluator;

impl AlertEvaluator {
    /// Evaluate alarm state and enrich measurement with fault flags in one call.
    pub fn evaluate(
        measurement: &mut DeviceMeasurement,
        config: &AlertConfiguration,
        previous_state: AlarmState,
    ) -> AlarmState {
        let is_short = Self::is_short_condition(measurement, config);
        measurement.is_short = is_short;

        if measurement.is_open || measurement.is_overload {
            return AlarmState::Open;
        }

        if is_short {
            return AlarmState::Short;
        }

        if measurement.mode != MeasurementMode::DcVoltage {
            return AlarmState::None;
        }

        let Some(value) = measurement.primary_value else {
            return AlarmState::None;
        };

        if config.dcv_high_alarm_enabled {
            let threshold = config.dcv_high_alarm_value;
            let clear_threshold = threshold - threshold.abs() * HYSTERESIS_FRACTION;
            if value > threshold {
                return AlarmState::HighAlarm;
            }
            if previous_state == AlarmState::HighAlarm && value > clear_threshold {
                return AlarmState::HighAlarm;
            }
        }

        if config.dcv_low_alarm_enabled {
            let threshold = config.dcv_low_alarm_value;
            let clear_threshold = threshold + threshold.abs() * HYSTERESIS_FRACTION;
            if value < threshold {
                return AlarmState::LowAlarm;
            }
            if previous_state == AlarmState::LowAlarm && value < clear_threshold {
                return AlarmState::LowAlarm;
            }
        }

        AlarmState::None
    }

    fn is_short_condition(measurement: &DeviceMeasurement, config: &AlertConfiguration) -> bool {
        if measurement.is_overload || measurement.is_open {
            return false;
        }
        let Some(value) = measurement.primary_value else {
            return false;
        };
        matches!(
            measurement.mode,
            MeasurementMode::Continuity | MeasurementMode::Resistance | MeasurementMode::Diode
        ) && value < config.short_threshold
    }
}
