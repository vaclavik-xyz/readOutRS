use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::DeviceId;
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

    // Read several measurements to accumulate energy
    for _ in 0..5 {
        let _ = driver.read_measurement().await;
    }

    let m = driver.read_measurement().await.unwrap();
    assert!(m.energy_mwh.is_some());
    assert!(m.energy_mah.is_some());
}
