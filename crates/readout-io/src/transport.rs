#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("not open")]
    NotOpen,
    #[error("connection lost: {0}")]
    ConnectionLost(String),
    #[error("timeout")]
    Timeout,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub trait DeviceTransport: Send + Sync {
    async fn open(&mut self) -> Result<(), TransportError>;
    async fn close(&mut self);
    async fn read_frame(&mut self) -> Result<Option<String>, TransportError>;
}

pub trait ScpiTransport: DeviceTransport {
    async fn query(&mut self, command: &str) -> Result<Option<String>, TransportError>;
}
