#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MeasurementMode {
    DcVoltage,
    AcVoltage,
    DcCurrent,
    AcCurrent,
    Resistance,
    Continuity,
    Diode,
    Capacitance,
    Frequency,
    Period,
    Temperature,
    Unknown,
}

pub struct MeasurementModeParser;

impl MeasurementModeParser {
    pub fn parse(mode_string: Option<&str>) -> MeasurementMode {
        let raw = match mode_string {
            Some(s) => s.trim().to_uppercase(),
            None => return MeasurementMode::Unknown,
        };

        Self::parse_normalized(&raw)
    }

    pub fn parse_normalized(raw: &str) -> MeasurementMode {
        if raw.is_empty() {
            return MeasurementMode::Unknown;
        }

        if raw.contains("VOLT") {
            return if raw.contains("AC") {
                MeasurementMode::AcVoltage
            } else {
                MeasurementMode::DcVoltage
            };
        }
        if raw.contains("CURR") {
            return if raw.contains("AC") {
                MeasurementMode::AcCurrent
            } else {
                MeasurementMode::DcCurrent
            };
        }
        if raw.contains("CONT") {
            return MeasurementMode::Continuity;
        }
        if raw.contains("RES") || raw.contains("OHM") || raw.contains("FRES") {
            return MeasurementMode::Resistance;
        }
        if raw.contains("DIOD") {
            return MeasurementMode::Diode;
        }
        if raw.contains("CAP") {
            return MeasurementMode::Capacitance;
        }
        if raw.contains("FREQ") {
            return MeasurementMode::Frequency;
        }
        // TEMP before PER: "TEMPERATURE" contains "PER"
        if raw.contains("TEMP") {
            return MeasurementMode::Temperature;
        }
        if raw.contains("PER") {
            return MeasurementMode::Period;
        }

        MeasurementMode::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normalized_skips_uppercase() {
        assert_eq!(MeasurementModeParser::parse_normalized("VOLT:DC"), MeasurementMode::DcVoltage);
        assert_eq!(MeasurementModeParser::parse_normalized("CURR:AC"), MeasurementMode::AcCurrent);
        assert_eq!(MeasurementModeParser::parse_normalized(""), MeasurementMode::Unknown);
    }
}
