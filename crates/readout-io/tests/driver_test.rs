use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{DeviceId, MultimeterRange, MultimeterRate};
use readout_io::multimeter_driver::MultimeterDriver;
use readout_io::simulated::*;
use readout_io::usbc_driver::UsbCDriver;

#[tokio::test]
async fn multimeter_driver_produces_measurements() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();

    let m = driver.poll().await.unwrap();
    assert_eq!(m.device, DeviceId::Multimeter);
    assert_eq!(m.mode, MeasurementMode::DcVoltage);
    assert!(m.primary_value.is_some());
    assert_eq!(m.primary_unit, "V DC");
}

#[tokio::test]
async fn multimeter_driver_mode_changes() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();
    // Mode starts as DcVoltage
    let m = driver.poll().await.unwrap();
    assert_eq!(m.mode, MeasurementMode::DcVoltage);
}

#[tokio::test]
async fn multimeter_driver_query_identity() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();
    let identity = driver.query_identity().await;
    assert!(identity.is_some());
    assert!(identity.unwrap().contains("SIMULATED"));
}

#[tokio::test]
async fn multimeter_driver_set_mode() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();
    driver.set_mode(MeasurementMode::Resistance).await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.mode, MeasurementMode::Resistance);
    assert!(state.auto_range);
}

#[tokio::test]
async fn multimeter_driver_set_range() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();
    driver.set_range(MultimeterRange::Manual(4)).await.unwrap();
    let state = driver.query_state().await;
    assert!(!state.auto_range);
    assert!(!state.range_label.is_empty());
    driver.set_range(MultimeterRange::Auto).await.unwrap();
    let state = driver.query_state().await;
    assert!(state.auto_range);
}

#[tokio::test]
async fn multimeter_driver_set_rate() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();
    driver.set_rate(MultimeterRate::Fast).await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.rate, MultimeterRate::Fast);
    driver.set_rate(MultimeterRate::Slow).await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.rate, MultimeterRate::Slow);
}

#[tokio::test]
async fn multimeter_driver_query_state() {
    let transport = SimulatedScpiTransport::new(10);
    let mut driver = MultimeterDriver::new(transport);
    driver.connect().await.unwrap();
    let state = driver.query_state().await;
    assert_eq!(state.mode, MeasurementMode::DcVoltage);
    assert_eq!(state.rate, MultimeterRate::Medium);
    assert!(state.auto_range);
    assert!(!state.range_label.is_empty());
}

#[tokio::test]
async fn usbc_driver_produces_measurements() {
    let transport = SimulatedStreamingTransport::new(10);
    let mut driver = UsbCDriver::new(transport);
    driver.connect().await.unwrap();

    let m = driver.read_measurement().await.unwrap();
    assert_eq!(m.device, DeviceId::UsbC);
    assert!(m.primary_value.is_some()); // voltage
    assert!(m.secondary_value.is_some()); // current
    assert!(m.power_watts.is_some());
}

#[tokio::test]
async fn usbc_driver_accumulates_energy() {
    let transport = SimulatedStreamingTransport::new(10);
    let mut driver = UsbCDriver::new(transport);
    driver.connect().await.unwrap();

    let first = driver.read_measurement().await.unwrap();
    for _ in 0..5 {
        driver.read_measurement().await.unwrap();
    }

    let m = driver.read_measurement().await.unwrap();
    let first_mwh = first.energy_mwh.expect("initial energy mWh");
    let first_mah = first.energy_mah.expect("initial energy mAh");
    let final_mwh = m.energy_mwh.expect("final energy mWh");
    let final_mah = m.energy_mah.expect("final energy mAh");

    assert!(
        final_mwh > first_mwh,
        "energy mWh did not increase: {final_mwh} <= {first_mwh}"
    );
    assert!(
        final_mah > first_mah,
        "energy mAh did not increase: {final_mah} <= {first_mah}"
    );
}
