use readout_persistence::config::*;
use readout_persistence::config_validator::*;

#[test]
fn valid_simulator_config_has_no_errors() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    let issues = ConfigValidator::validate(&config);
    assert!(issues.iter().all(|i| i.severity != IssueSeverity::Error));
}

#[test]
fn hardware_mode_without_port_is_error() {
    let mut config = AppConfiguration::default();
    config.use_simulator = false;
    config.multimeter_enabled = true;
    config.multimeter_port = String::new();
    let issues = ConfigValidator::validate(&config);
    assert!(issues.iter().any(|i| i.severity == IssueSeverity::Error));
}

#[test]
fn usbc_hardware_mode_without_port_is_error() {
    let mut config = AppConfiguration::default();
    config.use_simulator = false;
    config.usbc_enabled = true;
    config.usbc_port = String::new();

    let issues = ConfigValidator::validate(&config);

    assert!(
        issues
            .iter()
            .any(|i| { i.severity == IssueSeverity::Error && i.message.contains("USB-C meter") })
    );
}

#[test]
fn csv_enabled_without_path_is_warning() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.multimeter_csv_logging_enabled = true;
    config.multimeter_csv_log_file_path = String::new();
    let issues = ConfigValidator::validate(&config);
    assert!(
        issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Warning && i.message.contains("CSV"))
    );
}

#[test]
fn usbc_csv_enabled_without_path_is_warning() {
    let mut config = AppConfiguration::default();
    config.use_simulator = true;
    config.usbc_csv_logging_enabled = true;
    config.usbc_csv_log_file_path = String::new();

    let issues = ConfigValidator::validate(&config);

    assert!(
        issues
            .iter()
            .any(|i| { i.severity == IssueSeverity::Warning && i.message.contains("USB-C CSV") })
    );
}

#[test]
fn high_alarm_not_above_low_alarm_is_warning() {
    let mut config = AppConfiguration::default();
    config.dcv_high_alarm_enabled = true;
    config.dcv_low_alarm_enabled = true;
    config.dcv_high_alarm_value = 1.0;
    config.dcv_low_alarm_value = 1.0;

    let issues = ConfigValidator::validate(&config);

    assert!(issues.iter().any(|i| {
        i.severity == IssueSeverity::Warning && i.message.contains("High alarm threshold")
    }));
}
