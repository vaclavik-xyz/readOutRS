use crate::transport::{DeviceTransport, ScpiTransport, TransportError};

// --- SimulatedScpiTransport (Multimeter) ---

const MODES: [&str; 4] = ["VOLT:DC", "CURR:DC", "RES", "VOLT:AC"];
const MODE_CYCLE_SAMPLES: usize = 220;

pub struct SimulatedScpiTransport {
    sample_rate_hz: u32,
    is_open: bool,
    sample_index: usize,
    beeper_enabled: bool,
}

impl SimulatedScpiTransport {
    pub fn new(sample_rate_hz: u32) -> Self {
        assert!(sample_rate_hz > 0, "sample_rate_hz must be > 0");
        Self {
            sample_rate_hz,
            is_open: false,
            sample_index: 0,
            beeper_enabled: false,
        }
    }

    fn current_mode(&self) -> &'static str {
        let mode_idx = (self.sample_index / MODE_CYCLE_SAMPLES) % MODES.len();
        MODES[mode_idx]
    }

    fn generate_value(&self) -> String {
        let t = self.sample_index as f64 / self.sample_rate_hz as f64;
        let mode = self.current_mode();

        match mode {
            "VOLT:DC" => {
                let v = 12.0 + 0.25 * (t * 1.2).sin();
                format!("{v:.6E}")
            }
            "CURR:DC" => {
                let v = 1.4 + 0.3 * (t * 1.8 + 0.5).sin();
                format!("{v:.6E}")
            }
            "RES" => {
                // Return "OL" every 160 samples
                if self.sample_index % 160 == 0 && self.sample_index > 0 {
                    return "OL".into();
                }
                let v = 120.0 + 45.0 * (t * 0.8).sin();
                format!("{v:.6E}")
            }
            "VOLT:AC" => {
                let v = 230.0 + 8.0 * (t * 0.6).sin();
                format!("{v:.6E}")
            }
            _ => "0.0E+00".into(),
        }
    }
}

impl DeviceTransport for SimulatedScpiTransport {
    async fn open(&mut self) -> Result<(), TransportError> {
        self.is_open = true;
        self.sample_index = 0;
        Ok(())
    }

    async fn close(&mut self) {
        self.is_open = false;
    }

    async fn read_frame(&mut self) -> Result<Option<String>, TransportError> {
        if !self.is_open {
            return Err(TransportError::NotOpen);
        }
        // SCPI transport uses query(), not read_frame()
        Ok(None)
    }
}

impl ScpiTransport for SimulatedScpiTransport {
    async fn query(&mut self, command: &str) -> Result<Option<String>, TransportError> {
        if !self.is_open {
            return Err(TransportError::NotOpen);
        }

        let cmd = command.trim().to_uppercase();

        if cmd == "*IDN?" {
            return Ok(Some("SIMULATED,READOUT,MULTIMETER,1.0".into()));
        }

        if cmd == "FUNC?" {
            return Ok(Some(self.current_mode().into()));
        }

        if cmd.starts_with("MEAS") {
            let value = self.generate_value();
            self.sample_index += 1;
            // Simulate sample rate timing
            tokio::time::sleep(std::time::Duration::from_micros(
                1_000_000 / self.sample_rate_hz as u64,
            ))
            .await;
            return Ok(Some(value));
        }

        if cmd == "SYST:BEEP:STAT?" {
            return Ok(Some(if self.beeper_enabled { "1" } else { "0" }.into()));
        }

        if cmd.starts_with("SYST:BEEP:STAT") {
            self.beeper_enabled = cmd.ends_with(" ON");
            return Ok(None);
        }

        Ok(None)
    }
}

// --- SimulatedStreamingTransport (USB-C) ---

pub struct SimulatedStreamingTransport {
    sample_rate_hz: u32,
    is_open: bool,
    sample_index: usize,
}

impl SimulatedStreamingTransport {
    pub fn new(sample_rate_hz: u32) -> Self {
        assert!(sample_rate_hz > 0, "sample_rate_hz must be > 0");
        Self {
            sample_rate_hz,
            is_open: false,
            sample_index: 0,
        }
    }

    fn encode_frame(voltage: f64, current: f64) -> String {
        let bus_raw = ((voltage / 0.003125).round() as i64).clamp(0, 65535) as u16;
        let shunt_signed = ((current / 0.0002).round() as i64).clamp(-32768, 32767) as i16;
        let shunt_raw = shunt_signed as u16;
        format!("{shunt_raw:04X}{bus_raw:04X}")
    }
}

impl DeviceTransport for SimulatedStreamingTransport {
    async fn open(&mut self) -> Result<(), TransportError> {
        self.is_open = true;
        self.sample_index = 0;
        Ok(())
    }

    async fn close(&mut self) {
        self.is_open = false;
    }

    async fn read_frame(&mut self) -> Result<Option<String>, TransportError> {
        if !self.is_open {
            return Err(TransportError::NotOpen);
        }

        let t = self.sample_index as f64 / self.sample_rate_hz as f64;

        let voltage = (9.0 + 1.8 * (t * 0.65).sin()).clamp(3.3, 20.0);
        let current = (1.3 + 0.45 * (t * 1.05 + 0.7).sin()).clamp(0.0, 4.5);

        let frame = Self::encode_frame(voltage, current);
        self.sample_index += 1;

        // Simulate sample rate timing
        tokio::time::sleep(std::time::Duration::from_micros(
            1_000_000 / self.sample_rate_hz as u64,
        ))
        .await;

        Ok(Some(frame))
    }
}
