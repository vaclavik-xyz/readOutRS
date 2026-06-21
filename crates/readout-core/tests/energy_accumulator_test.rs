use readout_core::energy_accumulator::*;
use std::time::Duration;

#[test]
fn initial_state_is_zero() {
    let acc = EnergyAccumulator::new();
    assert_eq!(acc.energy_mwh(), 0.0);
    assert_eq!(acc.energy_mah(), 0.0);
}

#[test]
fn first_update_records_no_energy() {
    let mut acc = EnergyAccumulator::new();
    let snap = acc.update(5.0, 2.0, Duration::from_secs(0));
    assert_eq!(snap.energy_mwh, 0.0);
    assert_eq!(snap.energy_mah, 0.0);
    assert!((snap.power_watts - 10.0).abs() < 0.001);
}

#[test]
fn second_update_accumulates_energy() {
    let mut acc = EnergyAccumulator::new();
    acc.update(5.0, 2.0, Duration::from_secs(0));
    let snap = acc.update(5.0, 2.0, Duration::from_secs(3600)); // 1 hour later
    // 10W * 1h = 10Wh = 10000mWh
    assert!((snap.energy_mwh - 10000.0).abs() < 1.0);
    // 2A * 1h = 2Ah = 2000mAh
    assert!((snap.energy_mah - 2000.0).abs() < 1.0);
}

#[test]
fn reset_clears_accumulator() {
    let mut acc = EnergyAccumulator::new();
    acc.update(5.0, 2.0, Duration::from_secs(0));
    acc.update(5.0, 2.0, Duration::from_secs(3600));
    acc.reset();
    assert_eq!(acc.energy_mwh(), 0.0);
    assert_eq!(acc.energy_mah(), 0.0);

    let snap = acc.update(5.0, 2.0, Duration::from_secs(7200));
    assert!((snap.power_watts - 10.0).abs() < 0.001);
    assert_eq!(snap.energy_mwh, 0.0);
    assert_eq!(snap.energy_mah, 0.0);
}

#[test]
fn negative_voltage_uses_abs_power() {
    let mut acc = EnergyAccumulator::new();
    acc.update(-5.0, 2.0, Duration::from_secs(0));
    let snap = acc.update(-5.0, 2.0, Duration::from_secs(3600));
    assert!((snap.power_watts - 10.0).abs() < 0.001);
    assert!((snap.energy_mwh - 10000.0).abs() < 1.0);
    assert!((snap.energy_mah - 2000.0).abs() < 1.0);
}
