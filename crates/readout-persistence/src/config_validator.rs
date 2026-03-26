use crate::config::AppConfiguration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub message: String,
}

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(config: &AppConfiguration) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if !config.use_simulator {
            if config.multimeter_enabled && config.multimeter_port.is_empty() {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    message: "Multimeter enabled but no port configured".into(),
                });
            }
            if config.usbc_enabled && config.usbc_port.is_empty() {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    message: "USB-C meter enabled but no port configured".into(),
                });
            }
        }

        if config.multimeter_csv_logging_enabled
            && config.multimeter_csv_log_file_path.is_empty()
        {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                message: "Multimeter CSV logging enabled but no file path set".into(),
            });
        }

        if config.usbc_csv_logging_enabled && config.usbc_csv_log_file_path.is_empty() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                message: "USB-C CSV logging enabled but no file path set".into(),
            });
        }

        if config.dcv_high_alarm_enabled
            && config.dcv_low_alarm_enabled
            && config.dcv_high_alarm_value <= config.dcv_low_alarm_value
        {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                message: "High alarm threshold is not above low alarm threshold".into(),
            });
        }

        issues
    }
}
