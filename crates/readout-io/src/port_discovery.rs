use serialport::SerialPortType;

#[derive(Debug, Clone)]
pub struct PortCandidate {
    pub port_name: String,
    pub score: u32,
    pub hints: Vec<String>,
}

const KNOWN_VENDORS: &[(&str, u32)] = &[
    ("CH340", 10),
    ("CH341", 10),
    ("FTDI", 8),
    ("CP210", 8),
    ("CP2102", 9),
    ("CP2104", 9),
    ("PL2303", 7),
    ("Prolific", 7),
    ("Silicon Labs", 8),
    ("SILICON LABS", 8),
    ("wch.cn", 10),
];

pub struct PortDiscovery;

impl PortDiscovery {
    pub fn scan() -> Vec<PortCandidate> {
        let ports = match serialport::available_ports() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to enumerate serial ports: {e}");
                return Vec::new();
            }
        };

        ports
            .into_iter()
            .map(|info| {
                let (score, hints) = Self::score_port(&info.port_type);
                PortCandidate {
                    port_name: info.port_name,
                    score,
                    hints,
                }
            })
            .collect()
    }

    pub fn score_port(port_type: &SerialPortType) -> (u32, Vec<String>) {
        let mut score = 0u32;
        let mut hints = Vec::new();

        match port_type {
            SerialPortType::UsbPort(usb) => {
                score += 5; // USB ports are more likely to be our devices

                let searchable = format!(
                    "{} {} {}",
                    usb.manufacturer.as_deref().unwrap_or(""),
                    usb.product.as_deref().unwrap_or(""),
                    usb.serial_number.as_deref().unwrap_or("")
                )
                .to_uppercase();

                for (vendor, bonus) in KNOWN_VENDORS {
                    if searchable.contains(&vendor.to_uppercase()) {
                        score += bonus;
                        hints.push(format!("Matched: {vendor}"));
                    }
                }

                if let Some(product) = &usb.product {
                    hints.push(format!("Product: {product}"));
                }
                if let Some(manufacturer) = &usb.manufacturer {
                    hints.push(format!("Manufacturer: {manufacturer}"));
                }
            }
            SerialPortType::PciPort => {
                score += 1;
                hints.push("PCI port".into());
            }
            SerialPortType::BluetoothPort => {
                hints.push("Bluetooth".into());
            }
            SerialPortType::Unknown => {
                score += 2;
            }
        }

        (score, hints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_usb_port_with_known_vendor() {
        let port_type = SerialPortType::UsbPort(serialport::UsbPortInfo {
            vid: 0x1A86,
            pid: 0x7523,
            serial_number: None,
            manufacturer: Some("wch.cn".into()),
            product: Some("CH340".into()),
        });
        let (score, hints) = PortDiscovery::score_port(&port_type);
        assert!(score >= 15);
        assert!(hints.iter().any(|h| h.contains("CH340")));
    }

    #[test]
    fn score_unknown_port_is_low() {
        let (score, _) = PortDiscovery::score_port(&SerialPortType::Unknown);
        assert!(score <= 5);
    }

    #[test]
    fn score_bluetooth_is_zero() {
        let (score, _) = PortDiscovery::score_port(&SerialPortType::BluetoothPort);
        assert_eq!(score, 0);
    }
}
