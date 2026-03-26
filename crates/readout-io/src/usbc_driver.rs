use crate::transport::{DeviceTransport, TransportError};
use readout_core::energy_accumulator::EnergyAccumulator;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{DeviceId, DeviceMeasurement};
use readout_core::usbc_frame_parser::UsbCFrameParser;
use std::time::{Duration, Instant};

pub struct UsbCDriver<T: DeviceTransport> {
    transport: T,
    energy: EnergyAccumulator,
    start_time: Option<Instant>,
}

impl<T: DeviceTransport> UsbCDriver<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            energy: EnergyAccumulator::new(),
            start_time: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), TransportError> {
        self.transport.open().await?;
        self.start_time = Some(Instant::now());
        self.energy.reset();
        Ok(())
    }

    pub async fn read_measurement(&mut self) -> Result<DeviceMeasurement, TransportError> {
        let frame = self.transport.read_frame().await?;

        let frame_str = frame.ok_or(TransportError::Timeout)?;
        let parsed = UsbCFrameParser::parse(&frame_str)
            .ok_or_else(|| TransportError::ConnectionLost("invalid frame".into()))?;

        let elapsed = self
            .start_time
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO);

        let snap = self
            .energy
            .update(parsed.voltage, parsed.current, elapsed);

        Ok(DeviceMeasurement {
            timestamp: Instant::now(),
            device: DeviceId::UsbC,
            primary_value: Some(parsed.voltage),
            primary_unit: "V".into(),
            secondary_value: Some(parsed.current),
            secondary_unit: Some("A".into()),
            power_watts: Some(snap.power_watts),
            energy_mwh: Some(snap.energy_mwh),
            energy_mah: Some(snap.energy_mah),
            mode: MeasurementMode::DcVoltage,
            mode_string: "USB-C".into(),
            is_overload: false,
            is_open: false,
            is_short: false,
        })
    }

    pub fn reset_energy(&mut self) {
        self.energy.reset();
    }

    pub async fn close(&mut self) {
        self.transport.close().await;
    }
}
