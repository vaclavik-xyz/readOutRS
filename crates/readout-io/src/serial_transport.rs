use crate::transport::{DeviceTransport, ScpiTransport, TransportError};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

pub struct SerialTransport {
    port_name: String,
    baud_rate: u32,
    timeout: Duration,
    port: Option<BufReader<Box<dyn serialport::SerialPort>>>,
}

impl SerialTransport {
    pub fn new(port_name: String, baud_rate: u32) -> Self {
        Self {
            port_name,
            baud_rate,
            timeout: Duration::from_secs(2),
            port: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn get_port(&mut self) -> Result<&mut BufReader<Box<dyn serialport::SerialPort>>, TransportError> {
        self.port.as_mut().ok_or(TransportError::NotOpen)
    }
}

impl DeviceTransport for SerialTransport {
    async fn open(&mut self) -> Result<(), TransportError> {
        let port = serialport::new(&self.port_name, self.baud_rate)
            .timeout(self.timeout)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .open()
            .map_err(|e| TransportError::ConnectionLost(e.to_string()))?;

        self.port = Some(BufReader::new(port));
        Ok(())
    }

    async fn close(&mut self) {
        self.port = None;
    }

    async fn read_frame(&mut self) -> Result<Option<String>, TransportError> {
        let reader = self.get_port()?;
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(line.trim().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Err(TransportError::Timeout),
            Err(e) => Err(TransportError::Io(e)),
        }
    }
}

impl ScpiTransport for SerialTransport {
    async fn query(&mut self, command: &str) -> Result<Option<String>, TransportError> {
        {
            let reader = self.get_port()?;
            let port = reader.get_mut();
            port.write_all(format!("{command}\n").as_bytes())
                .map_err(|e| TransportError::ConnectionLost(e.to_string()))?;
            port.flush()
                .map_err(|e| TransportError::ConnectionLost(e.to_string()))?;
        }
        self.read_frame().await
    }
}
