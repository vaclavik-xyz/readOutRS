use readout_core::measurement_mode::*;
use readout_core::types::*;

#[test]
fn device_id_variants() {
    assert_ne!(DeviceId::Multimeter, DeviceId::UsbC);
}

#[test]
fn measurement_mode_parse_volt_dc() {
    assert_eq!(
        MeasurementModeParser::parse(Some("VOLT:DC")),
        MeasurementMode::DcVoltage
    );
}

#[test]
fn measurement_mode_parse_volt_ac() {
    assert_eq!(
        MeasurementModeParser::parse(Some("VOLT:AC")),
        MeasurementMode::AcVoltage
    );
}

#[test]
fn measurement_mode_parse_curr_dc() {
    assert_eq!(
        MeasurementModeParser::parse(Some("CURR:DC")),
        MeasurementMode::DcCurrent
    );
}

#[test]
fn measurement_mode_parse_curr_ac() {
    assert_eq!(
        MeasurementModeParser::parse(Some("CURR:AC")),
        MeasurementMode::AcCurrent
    );
}

#[test]
fn measurement_mode_parse_resistance() {
    assert_eq!(
        MeasurementModeParser::parse(Some("RES")),
        MeasurementMode::Resistance
    );
    assert_eq!(
        MeasurementModeParser::parse(Some("FRES")),
        MeasurementMode::Resistance
    );
    assert_eq!(
        MeasurementModeParser::parse(Some("OHM")),
        MeasurementMode::Resistance
    );
}

#[test]
fn measurement_mode_parse_continuity() {
    assert_eq!(
        MeasurementModeParser::parse(Some("CONT")),
        MeasurementMode::Continuity
    );
}

#[test]
fn measurement_mode_parse_diode() {
    assert_eq!(
        MeasurementModeParser::parse(Some("DIOD")),
        MeasurementMode::Diode
    );
}

#[test]
fn measurement_mode_parse_capacitance() {
    assert_eq!(
        MeasurementModeParser::parse(Some("CAP")),
        MeasurementMode::Capacitance
    );
}

#[test]
fn measurement_mode_parse_frequency() {
    assert_eq!(
        MeasurementModeParser::parse(Some("FREQ")),
        MeasurementMode::Frequency
    );
}

#[test]
fn measurement_mode_parse_period() {
    assert_eq!(
        MeasurementModeParser::parse(Some("PER")),
        MeasurementMode::Period
    );
}

#[test]
fn measurement_mode_parse_temperature() {
    assert_eq!(
        MeasurementModeParser::parse(Some("TEMP")),
        MeasurementMode::Temperature
    );
}

#[test]
fn measurement_mode_parse_unknown() {
    assert_eq!(
        MeasurementModeParser::parse(Some("GARBAGE")),
        MeasurementMode::Unknown
    );
    assert_eq!(
        MeasurementModeParser::parse(None),
        MeasurementMode::Unknown
    );
    assert_eq!(
        MeasurementModeParser::parse(Some("")),
        MeasurementMode::Unknown
    );
}

#[test]
fn measurement_mode_parse_case_insensitive() {
    assert_eq!(
        MeasurementModeParser::parse(Some("volt:dc")),
        MeasurementMode::DcVoltage
    );
    assert_eq!(
        MeasurementModeParser::parse(Some("  Curr:AC  ")),
        MeasurementMode::AcCurrent
    );
}

#[test]
fn device_measurement_default_flags() {
    let m = DeviceMeasurement {
        timestamp: std::time::Instant::now(),
        device: DeviceId::Multimeter,
        primary_value: Some(12.5),
        primary_unit: "V DC".into(),
        secondary_value: None,
        secondary_unit: None,
        power_watts: None,
        energy_mwh: None,
        energy_mah: None,
        mode: MeasurementMode::DcVoltage,
        mode_string: "VOLT:DC".into(),
        is_overload: false,
        is_open: false,
        is_short: false,
    };
    assert_eq!(m.primary_value, Some(12.5));
    assert!(!m.is_overload);
}

#[test]
fn device_measurement_overload_has_no_value() {
    let m = DeviceMeasurement {
        timestamp: std::time::Instant::now(),
        device: DeviceId::Multimeter,
        primary_value: None,
        primary_unit: "Ω".into(),
        secondary_value: None,
        secondary_unit: None,
        power_watts: None,
        energy_mwh: None,
        energy_mah: None,
        mode: MeasurementMode::Resistance,
        mode_string: "RES".into(),
        is_overload: true,
        is_open: true,
        is_short: false,
    };
    assert!(m.primary_value.is_none());
    assert!(m.is_overload);
}
