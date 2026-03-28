use crate::measurement_mode::{MeasurementMode, MeasurementModeParser};
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct MultimeterParsedMeasurement {
    pub mode: MeasurementMode,
    pub mode_string: String,
    pub value: Option<f64>,
    pub unit: String,
    pub is_overload: bool,
    pub is_open: bool,
}

static MODE_UNITS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        ("VOLT", "V"),
        ("VOLT:DC", "V DC"),
        ("VOLT:AC", "V AC"),
        ("CURR", "A"),
        ("CURR:DC", "A DC"),
        ("CURR:AC", "A AC"),
        ("RES", "Ω"),
        ("FRES", "Ω"),
        ("CAP", "F"),
        ("FREQ", "Hz"),
        ("PER", "s"),
        ("CONT", "Ω"),
        ("DIOD", "V"),
        ("TEMP", "°C"),
    ])
});

const OVERLOAD_THRESHOLD: f64 = 1e7;

pub struct MultimeterParser;

impl MultimeterParser {
    pub fn parse(response: Option<&str>, mode_string: &str) -> Option<MultimeterParsedMeasurement> {
        let response = response?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized_mode = mode_string.trim().to_uppercase();
        let mode = MeasurementModeParser::parse_normalized(&normalized_mode);
        let open_candidate = Self::is_open_candidate(mode);

        let upper = trimmed.to_uppercase();
        if upper.contains("OL") || upper.contains("OVER") {
            let unit = Self::resolved_unit(&normalized_mode, mode);
            return Some(MultimeterParsedMeasurement {
                mode,
                mode_string: normalized_mode,
                value: None,
                unit,
                is_overload: true,
                is_open: open_candidate,
            });
        }

        let first_segment = trimmed.split(',').next().unwrap_or(trimmed);
        let numeric = Self::extract_numeric_prefix(first_segment);

        let parsed_value = numeric.and_then(|s| s.parse::<f64>().ok());

        let Some(value) = parsed_value else {
            return Some(MultimeterParsedMeasurement {
                mode,
                mode_string: normalized_mode,
                value: None,
                unit: String::new(),
                is_overload: false,
                is_open: false,
            });
        };

        if Self::is_value_overload(value, mode) {
            let unit = Self::resolved_unit(&normalized_mode, mode);
            return Some(MultimeterParsedMeasurement {
                mode,
                mode_string: normalized_mode,
                value: None,
                unit,
                is_overload: true,
                is_open: open_candidate,
            });
        }

        let unit = Self::resolved_unit(&normalized_mode, mode);
        Some(MultimeterParsedMeasurement {
            mode,
            mode_string: normalized_mode,
            value: Some(value),
            unit,
            is_overload: false,
            is_open: false,
        })
    }

    pub fn is_value_overload(value: f64, mode: MeasurementMode) -> bool {
        match mode {
            MeasurementMode::Diode | MeasurementMode::Resistance | MeasurementMode::Continuity => {
                value.abs() >= OVERLOAD_THRESHOLD
            }
            _ => value.abs() >= 1e30,
        }
    }

    fn is_open_candidate(mode: MeasurementMode) -> bool {
        matches!(
            mode,
            MeasurementMode::Resistance | MeasurementMode::Continuity | MeasurementMode::Diode
        )
    }

    fn resolved_unit(mode_string: &str, mode: MeasurementMode) -> String {
        if let Some(&unit) = MODE_UNITS.get(mode_string) {
            return unit.to_string();
        }
        match mode {
            MeasurementMode::DcVoltage => "V DC",
            MeasurementMode::AcVoltage => "V AC",
            MeasurementMode::DcCurrent => "A DC",
            MeasurementMode::AcCurrent => "A AC",
            MeasurementMode::Resistance | MeasurementMode::Continuity => "Ω",
            MeasurementMode::Diode => "V",
            MeasurementMode::Capacitance => "F",
            MeasurementMode::Frequency => "Hz",
            MeasurementMode::Temperature => "°C",
            MeasurementMode::Period => "s",
            MeasurementMode::Unknown => "",
        }
        .to_string()
    }

    fn extract_numeric_prefix(text: &str) -> Option<&str> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let mut end = 0;
        let bytes = text.as_bytes();

        // Optional sign
        if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
            end += 1;
        }
        // Digits
        let digit_start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == digit_start {
            return None;
        }
        // Optional decimal
        if end < bytes.len() && bytes[end] == b'.' {
            end += 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
        }
        // Optional exponent
        if end < bytes.len() && (bytes[end] == b'E' || bytes[end] == b'e') {
            end += 1;
            if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
                end += 1;
            }
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
        }

        if end > 0 { Some(&text[..end]) } else { None }
    }
}
