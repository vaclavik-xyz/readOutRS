pub const VOLTAGE_QUANTUM: f64 = 0.003125;
pub const CURRENT_QUANTUM: f64 = 0.0002;
pub const FRAME_LENGTH: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct UsbCFrameMeasurement {
    pub voltage: f64,
    pub current: f64,
}

pub struct UsbCFrameParser;

impl UsbCFrameParser {
    pub fn is_valid_frame(raw_frame: &str) -> bool {
        let frame = raw_frame.trim();
        frame.len() == FRAME_LENGTH && u32::from_str_radix(frame, 16).is_ok()
    }

    pub fn parse(raw_frame: &str) -> Option<UsbCFrameMeasurement> {
        let frame = raw_frame.trim().trim_matches(|c: char| !c.is_ascii_hexdigit());
        if !Self::is_valid_frame(frame) {
            return None;
        }

        let shunt_hex = &frame[..4];
        let bus_hex = &frame[4..];

        let shunt_raw = u16::from_str_radix(shunt_hex, 16).ok()?;
        let bus_raw = u16::from_str_radix(bus_hex, 16).ok()?;

        // Signed conversion for shunt
        let shunt_signed: i16 = shunt_raw as i16;

        let voltage = f64::from(bus_raw) * VOLTAGE_QUANTUM;
        let current = (f64::from(shunt_signed) * CURRENT_QUANTUM).max(0.0);

        Some(UsbCFrameMeasurement { voltage, current })
    }
}
