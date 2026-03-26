/// Format a value with the best SI prefix for its magnitude.
///
/// Returns `(scaled_value_string, prefix)` where prefix is e.g. "n", "µ", "m", "k", "M".
/// The base unit is NOT included — the caller appends it.
///
/// Examples:
///   format_si(0.000000123, "F") => "123.0 nF"
///   format_si(4700.0, "Ω")     => "4.700 kΩ"
///   format_si(12.345, "V DC")   => "12.35 V DC"
pub fn format_si(value: f64, unit: &str) -> String {
    if !value.is_finite() {
        return format!("OL {unit}");
    }

    let abs = value.abs();

    // Choose best prefix
    let (divisor, prefix) = if abs == 0.0 {
        (1.0, "")
    } else if abs >= 1e9 {
        (1e9, "G")
    } else if abs >= 1e6 {
        (1e6, "M")
    } else if abs >= 1e3 {
        (1e3, "k")
    } else if abs >= 1.0 {
        (1.0, "")
    } else if abs >= 1e-3 {
        (1e-3, "m")
    } else if abs >= 1e-6 {
        (1e-6, "µ")
    } else if abs >= 1e-9 {
        (1e-9, "n")
    } else if abs >= 1e-12 {
        (1e-12, "p")
    } else {
        (1.0, "")
    };

    let scaled = value / divisor;

    // Pick decimal places based on magnitude of scaled value
    let decimals = if scaled.abs() >= 99.95 {
        1
    } else if scaled.abs() >= 9.995 {
        2
    } else {
        3
    };

    format!("{scaled:.decimals$} {prefix}{unit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_units() {
        assert_eq!(format_si(12.345, "V DC"), "12.35 V DC");
        assert_eq!(format_si(0.230, "V AC"), "230.0 mV AC");
        assert_eq!(format_si(4700.0, "Ω"), "4.700 kΩ");
    }

    #[test]
    fn capacitance() {
        assert_eq!(format_si(1e-7, "F"), "100.0 nF");
        assert_eq!(format_si(4.7e-6, "F"), "4.700 µF");
        assert_eq!(format_si(1e-12, "F"), "1.000 pF");
    }

    #[test]
    fn current() {
        assert_eq!(format_si(0.0015, "A DC"), "1.500 mA DC");
        assert_eq!(format_si(2.5, "A DC"), "2.500 A DC");
    }

    #[test]
    fn zero_and_special() {
        assert_eq!(format_si(0.0, "V DC"), "0.000 V DC");
        assert_eq!(format_si(f64::INFINITY, "V"), "OL V");
        assert_eq!(format_si(f64::NAN, "Ω"), "OL Ω");
    }

    #[test]
    fn negative_values() {
        assert_eq!(format_si(-0.0234, "V DC"), "-23.40 mV DC");
    }

    #[test]
    fn large_resistance() {
        assert_eq!(format_si(1_500_000.0, "Ω"), "1.500 MΩ");
    }
}
