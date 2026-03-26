use readout_core::measurement_mode::MeasurementMode;
use readout_core::multimeter_parser::*;

#[derive(serde::Deserialize)]
struct FixtureExpected {
    mode: String,
    value: Option<f64>,
    unit: String,
    #[serde(rename = "isOverload")]
    is_overload: bool,
    #[serde(rename = "isOpen")]
    is_open: bool,
}

#[derive(serde::Deserialize)]
struct Fixture {
    response: String,
    mode: String,
    expected: FixtureExpected,
}

fn load_fixtures() -> Vec<Fixture> {
    let data = include_str!("fixtures/multimeter_fixtures.json");
    serde_json::from_str(data).expect("valid fixture JSON")
}

fn mode_name(mode: MeasurementMode) -> &'static str {
    match mode {
        MeasurementMode::DcVoltage => "dcVoltage",
        MeasurementMode::AcVoltage => "acVoltage",
        MeasurementMode::DcCurrent => "dcCurrent",
        MeasurementMode::AcCurrent => "acCurrent",
        MeasurementMode::Resistance => "resistance",
        MeasurementMode::Continuity => "continuity",
        MeasurementMode::Diode => "diode",
        MeasurementMode::Capacitance => "capacitance",
        MeasurementMode::Frequency => "frequency",
        MeasurementMode::Period => "period",
        MeasurementMode::Temperature => "temperature",
        MeasurementMode::Unknown => "unknown",
    }
}

#[test]
fn fixture_driven_parsing() {
    let fixtures = load_fixtures();
    for (i, f) in fixtures.iter().enumerate() {
        let result = MultimeterParser::parse(Some(&f.response), &f.mode);
        let result = result.unwrap_or_else(|| {
            panic!(
                "fixture {i}: parse returned None for response={:?} mode={:?}",
                f.response, f.mode
            )
        });

        assert_eq!(
            mode_name(result.mode),
            f.expected.mode,
            "fixture {i}: mode mismatch"
        );
        assert_eq!(
            result.value, f.expected.value,
            "fixture {i}: value mismatch for response={:?}",
            f.response
        );
        assert_eq!(
            result.unit, f.expected.unit,
            "fixture {i}: unit mismatch"
        );
        assert_eq!(
            result.is_overload, f.expected.is_overload,
            "fixture {i}: isOverload mismatch"
        );
        assert_eq!(
            result.is_open, f.expected.is_open,
            "fixture {i}: isOpen mismatch"
        );
    }
}

#[test]
fn parse_none_returns_none() {
    assert!(MultimeterParser::parse(None, "VOLT:DC").is_none());
}

#[test]
fn parse_empty_returns_none() {
    assert!(MultimeterParser::parse(Some(""), "VOLT:DC").is_none());
}

#[test]
fn parse_whitespace_returns_none() {
    assert!(MultimeterParser::parse(Some("   "), "VOLT:DC").is_none());
}

#[test]
fn value_overload_resistance_threshold() {
    assert!(MultimeterParser::is_value_overload(
        1e7,
        MeasurementMode::Resistance
    ));
    assert!(!MultimeterParser::is_value_overload(
        9.9e6,
        MeasurementMode::Resistance
    ));
}

#[test]
fn value_overload_voltage_threshold() {
    assert!(MultimeterParser::is_value_overload(
        1e30,
        MeasurementMode::DcVoltage
    ));
    assert!(!MultimeterParser::is_value_overload(
        999.0,
        MeasurementMode::DcVoltage
    ));
}
