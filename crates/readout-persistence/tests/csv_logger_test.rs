use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{DeviceId, DeviceMeasurement};
use readout_persistence::csv_logger::*;
use std::time::Instant;

#[tokio::test]
async fn csv_logger_writes_header_and_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.csv");

    let mut logger = CsvLogger::new(path.clone());
    logger.start();

    let m = DeviceMeasurement {
        timestamp: Instant::now(),
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
        alarm_state: readout_core::types::AlarmState::None,
    };

    logger.log(&m);
    logger.flush().await;

    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines[0].starts_with("timestamp,"));
    assert!(lines.len() >= 2);
    assert!(lines[1].contains("Multimeter"));
    assert!(lines[1].contains("12.5"));
}
