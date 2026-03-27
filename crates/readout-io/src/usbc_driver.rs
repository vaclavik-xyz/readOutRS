use crate::transport::{DeviceTransport, TransportError};
use readout_core::energy_accumulator::EnergyAccumulator;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{DeviceId, DeviceMeasurement};
use readout_core::usbc_frame_parser::UsbCFrameParser;
use std::time::{Duration, Instant};

const USBC_UNIT_V: &str = "V";
const USBC_UNIT_A: &str = "A";
const USBC_MODE_STRING: &str = "USB-C";

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
        // Read the first frame to verify the device is actually responding.
        // This also discards a likely partial read from opening mid-stream.
        match self.transport.read_frame().await {
            Ok(Some(_)) => {}
            Ok(None) => {
                self.transport.close().await;
                return Err(TransportError::Timeout);
            }
            Err(e) => {
                self.transport.close().await;
                return Err(e);
            }
        }
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
            primary_unit: USBC_UNIT_V.into(),
            secondary_value: Some(parsed.current),
            secondary_unit: Some(USBC_UNIT_A.into()),
            power_watts: Some(snap.power_watts),
            energy_mwh: Some(snap.energy_mwh),
            energy_mah: Some(snap.energy_mah),
            mode: MeasurementMode::DcVoltage,
            mode_string: USBC_MODE_STRING.into(),
            is_overload: false,
            is_open: false,
            is_short: false,
            alarm_state: readout_core::types::AlarmState::None,
        })
    }

    pub fn reset_energy(&mut self) {
        self.energy.reset();
    }

    pub async fn close(&mut self) {
        self.transport.close().await;
    }
}
