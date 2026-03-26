use readout_core::alerts::*;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::*;
use std::time::Instant;

fn make_measurement(mode: MeasurementMode, value: Option<f64>) -> DeviceMeasurement {
    DeviceMeasurement {
        timestamp: Instant::now(),
        device: DeviceId::Multimeter,
        primary_value: value,
        primary_unit: "V DC".into(),
        secondary_value: None,
        secondary_unit: None,
        power_watts: None,
        energy_mwh: None,
        energy_mah: None,
        mode,
        mode_string: "VOLT:DC".into(),
        is_overload: false,
        is_open: false,
        is_short: false,
    }
}

fn default_config() -> AlertConfiguration {
    AlertConfiguration {
        short_threshold: 2.0,
        dcv_high_alarm_enabled: true,
        dcv_high_alarm_value: 12.0,
        dcv_low_alarm_enabled: true,
        dcv_low_alarm_value: 3.0,
    }
}

#[test]
fn no_alarm_for_normal_dc_voltage() {
    let m = make_measurement(MeasurementMode::DcVoltage, Some(5.0));
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::None);
}

#[test]
fn high_alarm_triggered() {
    let m = make_measurement(MeasurementMode::DcVoltage, Some(13.0));
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::HighAlarm);
}

#[test]
fn low_alarm_triggered() {
    let m = make_measurement(MeasurementMode::DcVoltage, Some(2.0));
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::LowAlarm);
}

#[test]
fn high_alarm_hysteresis_holds() {
    let config = default_config();
    // Value just below threshold but within hysteresis band
    let clear_threshold = 12.0 * (1.0 - 0.01);
    let m = make_measurement(MeasurementMode::DcVoltage, Some(clear_threshold + 0.01));
    let state = AlertEvaluator::evaluate(&m, &config, AlarmState::HighAlarm);
    assert_eq!(state, AlarmState::HighAlarm); // Holds due to hysteresis
}

#[test]
fn high_alarm_clears_below_hysteresis() {
    let config = default_config();
    let clear_threshold = 12.0 * (1.0 - 0.01);
    let m = make_measurement(MeasurementMode::DcVoltage, Some(clear_threshold - 0.1));
    let state = AlertEvaluator::evaluate(&m, &config, AlarmState::HighAlarm);
    assert_eq!(state, AlarmState::None);
}

#[test]
fn open_on_overload() {
    let mut m = make_measurement(MeasurementMode::Resistance, None);
    m.is_overload = true;
    m.is_open = true;
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::Open);
}

#[test]
fn short_on_low_resistance() {
    let mut m = make_measurement(MeasurementMode::Continuity, Some(0.5));
    m.mode = MeasurementMode::Continuity;
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::Short);
}

#[test]
fn no_short_above_threshold() {
    let m = make_measurement(MeasurementMode::Continuity, Some(5.0));
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::None);
}

#[test]
fn no_alarm_for_non_dcv_mode() {
    let m = make_measurement(MeasurementMode::AcVoltage, Some(300.0));
    let state = AlertEvaluator::evaluate(&m, &default_config(), AlarmState::None);
    assert_eq!(state, AlarmState::None);
}

#[test]
fn enrich_stamps_short_flag() {
    let m = make_measurement(MeasurementMode::Continuity, Some(0.5));
    let config = default_config();
    let enriched = AlertEvaluator::enrich_measurement(m, &config);
    assert!(enriched.is_short);
}

#[test]
fn enrich_does_not_stamp_when_above_threshold() {
    let m = make_measurement(MeasurementMode::Continuity, Some(5.0));
    let config = default_config();
    let enriched = AlertEvaluator::enrich_measurement(m, &config);
    assert!(!enriched.is_short);
}
