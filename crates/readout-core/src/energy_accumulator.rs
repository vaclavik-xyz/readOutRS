use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct EnergySnapshot {
    pub power_watts: f64,
    pub energy_mwh: f64,
    pub energy_mah: f64,
}

pub struct EnergyAccumulator {
    energy_mwh: f64,
    energy_mah: f64,
    last_timestamp: Option<Duration>,
}

impl EnergyAccumulator {
    pub fn new() -> Self {
        Self {
            energy_mwh: 0.0,
            energy_mah: 0.0,
            last_timestamp: None,
        }
    }

    pub fn energy_mwh(&self) -> f64 {
        self.energy_mwh
    }

    pub fn energy_mah(&self) -> f64 {
        self.energy_mah
    }

    pub fn reset(&mut self) {
        self.energy_mwh = 0.0;
        self.energy_mah = 0.0;
        self.last_timestamp = None;
    }

    pub fn update(&mut self, voltage: f64, current: f64, timestamp: Duration) -> EnergySnapshot {
        let power = (voltage * current).abs();

        if let Some(prev) = self.last_timestamp {
            if let Some(delta) = timestamp.checked_sub(prev) {
                let delta_hours = delta.as_secs_f64() / 3600.0;
                self.energy_mwh += power * 1000.0 * delta_hours;
                self.energy_mah += current.abs() * 1000.0 * delta_hours;
            }
        }

        self.last_timestamp = Some(timestamp);

        EnergySnapshot {
            power_watts: power,
            energy_mwh: self.energy_mwh,
            energy_mah: self.energy_mah,
        }
    }
}

impl Default for EnergyAccumulator {
    fn default() -> Self {
        Self::new()
    }
}
