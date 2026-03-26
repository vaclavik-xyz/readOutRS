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

    fn take_port(&mut self) -> Result<BufReader<Box<dyn serialport::SerialPort>>, TransportError> {
        self.port.take().ok_or(TransportError::NotOpen)
    }

    fn put_port(&mut self, port: BufReader<Box<dyn serialport::SerialPort>>) {
        self.port = Some(port);
    }
}

impl DeviceTransport for SerialTransport {
    async fn open(&mut self) -> Result<(), TransportError> {
        let port_name = self.port_name.clone();
        let baud_rate = self.baud_rate;
        let timeout = self.timeout;

        let port = tokio::task::spawn_blocking(move || {
            serialport::new(&port_name, baud_rate)
                .timeout(timeout)
                .data_bits(serialport::DataBits::Eight)
                .parity(serialport::Parity::None)
                .stop_bits(serialport::StopBits::One)
                .open()
                .map_err(|e| TransportError::ConnectionLost(e.to_string()))
        })
        .await
        .map_err(|e| TransportError::ConnectionLost(e.to_string()))??;

        self.port = Some(BufReader::new(port));
        Ok(())
    }

    async fn close(&mut self) {
        self.port = None;
    }

    async fn read_frame(&mut self) -> Result<Option<String>, TransportError> {
        let mut reader = self.take_port()?;

        let result = tokio::task::spawn_blocking(move || {
            let mut line = String::new();
            let res = match reader.read_line(&mut line) {
                Ok(0) => Ok(None),
                Ok(_) => Ok(Some(line.trim().to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Err(TransportError::Timeout),
                Err(e) => Err(TransportError::Io(e)),
            };
            (reader, res)
        })
        .await
        .map_err(|e| TransportError::ConnectionLost(e.to_string()))?;

        let (reader, res) = result;
        self.put_port(reader);
        res
    }
}

impl ScpiTransport for SerialTransport {
    async fn query(&mut self, command: &str) -> Result<Option<String>, TransportError> {
        let cmd = format!("{command}\n");
        let mut reader = self.take_port()?;

        let result = tokio::task::spawn_blocking(move || {
            let port = reader.get_mut();
            if let Err(e) = port.write_all(cmd.as_bytes()) {
                return (reader, Err(TransportError::ConnectionLost(e.to_string())));
            }
            if let Err(e) = port.flush() {
                return (reader, Err(TransportError::ConnectionLost(e.to_string())));
            }

            let mut line = String::new();
            let res = match reader.read_line(&mut line) {
                Ok(0) => Ok(None),
                Ok(_) => Ok(Some(line.trim().to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Err(TransportError::Timeout),
                Err(e) => Err(TransportError::Io(e)),
            };
            (reader, res)
        })
        .await
        .map_err(|e| TransportError::ConnectionLost(e.to_string()))?;

        let (reader, res) = result;
        self.put_port(reader);
        res
    }
}
