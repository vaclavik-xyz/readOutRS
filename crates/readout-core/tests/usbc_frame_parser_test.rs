use readout_core::usbc_frame_parser::*;

#[derive(serde::Deserialize)]
struct Fixture {
    frame: String,
    valid: bool,
    #[serde(rename = "expectedVoltage")]
    expected_voltage: Option<f64>,
    #[serde(rename = "expectedCurrent")]
    expected_current: Option<f64>,
}

fn load_fixtures() -> Vec<Fixture> {
    let data = include_str!("fixtures/usbc_frame_fixtures.json");
    serde_json::from_str(data).expect("valid fixture JSON")
}

#[test]
fn fixture_driven_parsing() {
    let fixtures = load_fixtures();
    for (i, f) in fixtures.iter().enumerate() {
        assert_eq!(
            UsbCFrameParser::is_valid_frame(&f.frame),
            f.valid,
            "fixture {i}: validity mismatch for frame={:?}",
            f.frame
        );

        let result = UsbCFrameParser::parse(&f.frame);
        if f.valid {
            let m = result.unwrap_or_else(|| panic!("fixture {i}: expected Some for valid frame"));
            let expected_v = f.expected_voltage.unwrap();
            let expected_c = f.expected_current.unwrap();
            assert!(
                (m.voltage - expected_v).abs() < 0.001,
                "fixture {i}: voltage mismatch: got {} expected {}",
                m.voltage,
                expected_v
            );
            assert!(
                (m.current - expected_c).abs() < 0.001,
                "fixture {i}: current mismatch: got {} expected {}",
                m.current,
                expected_c
            );
        } else {
            assert!(
                result.is_none(),
                "fixture {i}: expected None for invalid frame"
            );
        }
    }
}

#[test]
fn negative_current_clamped_to_zero() {
    let frame = "80000BB8"; // shunt = 0x8000 = 32768 -> signed = -32768
    let result = UsbCFrameParser::parse(frame).unwrap();
    assert_eq!(result.current, 0.0);
}

#[test]
fn whitespace_trimmed() {
    let result = UsbCFrameParser::parse("  03E80BB8  ");
    assert!(result.is_some());
}
