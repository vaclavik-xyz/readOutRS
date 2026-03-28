use crate::transport::{DeviceTransport, ScpiTransport, TransportError};

// --- SimulatedScpiTransport (Multimeter) ---

const DEFAULT_MODE: &str = "VOLT:DC";

pub struct SimulatedScpiTransport {
    sample_rate_hz: u32,
    is_open: bool,
    sample_index: usize,
    beeper_enabled: bool,
    mode: String,
    auto_range: bool,
    range_index: u8,
    rate: char,
    dual_display: bool,
    null_enabled: bool,
    dc_filter: bool,
    auto_impedance: bool,
    calc_active: bool,
    calc_func: String,
}

impl SimulatedScpiTransport {
    pub fn new(sample_rate_hz: u32) -> Self {
        assert!(sample_rate_hz > 0, "sample_rate_hz must be > 0");
        Self {
            sample_rate_hz,
            is_open: false,
            sample_index: 0,
            beeper_enabled: false,
            mode: DEFAULT_MODE.into(),
            auto_range: true,
            range_index: 3,
            rate: 'M',
            dual_display: false,
            null_enabled: false,
            dc_filter: false,
            auto_impedance: false,
            calc_active: false,
            calc_func: String::new(),
        }
    }

    fn generate_value(&self) -> String {
        let t = self.sample_index as f64 / self.sample_rate_hz as f64;
        match self.mode.as_str() {
            "VOLT:DC" => format!("{:.6E}", 12.0 + 0.25 * (t * 1.2).sin()),
            "CURR:DC" => format!("{:.6E}", 1.4 + 0.3 * (t * 1.8 + 0.5).sin()),
            "RES" | "FRES" => {
                if self.sample_index.is_multiple_of(160) && self.sample_index > 0 {
                    return "OL".into();
                }
                format!("{:.6E}", 120.0 + 45.0 * (t * 0.8).sin())
            }
            "VOLT:AC" => format!("{:.6E}", 230.0 + 8.0 * (t * 0.6).sin()),
            "CURR:AC" => format!("{:.6E}", 0.5 + 0.15 * (t * 1.0).sin()),
            "CAP" => format!("{:.6E}", 4.7e-6 + 0.2e-6 * (t * 0.5).sin()),
            "FREQ" => format!("{:.6E}", 50.0 + 0.1 * (t * 0.3).sin()),
            "PER" => format!("{:.6E}", 0.02 + 0.0001 * (t * 0.3).sin()),
            "CONT" => format!("{:.6E}", 0.5 + 0.3 * (t * 2.0).sin()),
            "DIOD" => format!("{:.6E}", 0.65 + 0.02 * (t * 0.8).sin()),
            "TEMP" => format!("{:.6E}", 23.0 + 1.5 * (t * 0.2).sin()),
            _ => "0.0E+00".into(),
        }
    }

    fn range_label(&self) -> String {
        let labels: &[&str] = match self.mode.as_str() {
            "VOLT:DC" => &["50 mV", "500 mV", "5 V", "50 V", "500 V", "1000 V"],
            "VOLT:AC" => &["500 mV", "5 V", "50 V", "500 V", "750 V"],
            "CURR:DC" | "CURR:AC" => &["500 uA", "5 mA", "50 mA", "500 mA", "5 A", "10 A"],
            "RES" | "FRES" => &[
                "500 OHM", "5 kOHM", "50 kOHM", "500 kOHM", "5 MOHM", "50 MOHM",
            ],
            "CAP" => &[
                "50 nF", "500 nF", "5 uF", "50 uF", "500 uF", "5 mF", "50 mF",
            ],
            _ => &["---"],
        };
        let idx = (self.range_index as usize)
            .saturating_sub(1)
            .min(labels.len() - 1);
        labels[idx].into()
    }

    fn handle_conf(&mut self, cmd: &str) {
        let new_mode = if cmd.contains("VOLT:DC") || cmd == "CONF:VOLT" {
            "VOLT:DC"
        } else if cmd.contains("VOLT:AC") {
            "VOLT:AC"
        } else if cmd.contains("CURR:DC") || cmd == "CONF:CURR" {
            "CURR:DC"
        } else if cmd.contains("CURR:AC") {
            "CURR:AC"
        } else if cmd.contains("FRES") {
            "FRES"
        } else if cmd.contains("RES") {
            "RES"
        } else if cmd.contains("CAP") {
            "CAP"
        } else if cmd.contains("FREQ") {
            "FREQ"
        } else if cmd.contains("PER") {
            "PER"
        } else if cmd.contains("DIOD") {
            "DIOD"
        } else if cmd.contains("CONT") {
            "CONT"
        } else if cmd.contains("TEMP") {
            "TEMP"
        } else {
            return;
        };
        self.mode = new_mode.into();
        self.auto_range = true;
        self.range_index = 3;
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
        if cmd == "FUNC?" || cmd == "FUNC1?" {
            return Ok(Some(self.mode.clone()));
        }
        if cmd == "MEAS2?" {
            if self.dual_display {
                let t = self.sample_index as f64 / self.sample_rate_hz as f64;
                return Ok(Some(format!("{:.6E}", 50.0 + 0.1 * (t * 0.3).sin())));
            }
            return Ok(None);
        }
        if cmd.starts_with("MEAS") {
            let value = self.generate_value();
            self.sample_index += 1;
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
        if cmd.starts_with("CONF:") {
            self.handle_conf(&cmd);
            return Ok(None);
        }
        if cmd == "AUTO" {
            self.auto_range = true;
            return Ok(None);
        }
        if cmd == "AUTO?" {
            return Ok(Some(if self.auto_range { "1" } else { "0" }.into()));
        }
        if cmd.starts_with("RANGE ") {
            if let Some(n) = cmd
                .strip_prefix("RANGE ")
                .and_then(|s| s.parse::<u8>().ok())
            {
                self.range_index = n.clamp(1, 7);
                self.auto_range = false;
            }
            return Ok(None);
        }
        if cmd == "RANGE?" {
            return Ok(Some(self.range_label()));
        }
        if cmd.starts_with("RATE ") {
            if let Some(c) = cmd.strip_prefix("RATE ").and_then(|s| s.chars().next())
                && matches!(c, 'F' | 'M' | 'S')
            {
                self.rate = c;
            }
            return Ok(None);
        }
        if cmd == "RATE?" {
            return Ok(Some(self.rate.to_string()));
        }
        // Dual display
        if cmd.starts_with("FUNC2") {
            if cmd == "FUNC2?" {
                return Ok(Some(if self.dual_display { "FREQ" } else { "NONe" }.into()));
            }
            // Strip quotes and compare case-insensitively
            self.dual_display = cmd.replace('"', "").contains("FREQ");
            return Ok(None);
        }
        // NULL
        if cmd.ends_with(":NULL?") {
            return Ok(Some(if self.null_enabled { "1" } else { "0" }.into()));
        }
        if cmd.ends_with(":NULL ON") || cmd.ends_with(":NULL OFF") {
            self.null_enabled = cmd.ends_with(" ON");
            return Ok(None);
        }
        // DC filter
        if cmd.starts_with("VOLT:DC:FILT") {
            if cmd == "VOLT:DC:FILT?" {
                return Ok(Some(if self.dc_filter { "1" } else { "0" }.into()));
            }
            self.dc_filter = cmd.ends_with(" ON");
            return Ok(None);
        }
        // Auto impedance
        if cmd.starts_with("VOLT:DC:IMP:AUTO") {
            if cmd == "VOLT:DC:IMP:AUTO?" {
                return Ok(Some(if self.auto_impedance { "1" } else { "0" }.into()));
            }
            self.auto_impedance = cmd.ends_with(" ON");
            return Ok(None);
        }
        // Continuity threshold
        if cmd.starts_with("CONT:THRE") {
            return Ok(None);
        }
        // Temperature config
        if cmd.starts_with("TEMP:RTD:") {
            return Ok(None);
        }
        // Math/calc
        if cmd.starts_with("CALC:") {
            if cmd == "CALC:STAT OFF" {
                self.calc_active = false;
                return Ok(None);
            }
            if cmd == "CALC:STAT ON" {
                self.calc_active = true;
                return Ok(None);
            }
            if cmd.starts_with("CALC:FUNC") {
                if let Some(func) = cmd.strip_prefix("CALC:FUNC ") {
                    self.calc_func = func.to_string();
                }
                return Ok(None);
            }
            if cmd == "CALC:AVER:ALL?" {
                if self.calc_active {
                    let t = self.sample_index as f64 / self.sample_rate_hz as f64;
                    let avg = 12.0 + 0.25 * (t * 1.2).sin();
                    return Ok(Some(format!(
                        "{:.6},{:.6},{:.6},{}",
                        avg - 0.5,
                        avg + 0.3,
                        avg,
                        self.sample_index
                    )));
                }
                return Ok(None);
            }
            // DB/DBM reference (must be inside CALC: block)
            if cmd.starts_with("CALC:DB:REF") || cmd.starts_with("CALC:DBM:REF") {
                return Ok(None);
            }
            return Ok(None);
        }
        // Remote mode
        if cmd == "SYST:REM" || cmd == "SYST:LOC" {
            return Ok(None);
        }
        // Device reset
        if cmd == "*RST" {
            self.mode = DEFAULT_MODE.into();
            self.auto_range = true;
            self.range_index = 3;
            self.rate = 'M';
            self.dual_display = false;
            self.null_enabled = false;
            self.dc_filter = false;
            self.auto_impedance = false;
            self.calc_active = false;
            self.calc_func.clear();
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
