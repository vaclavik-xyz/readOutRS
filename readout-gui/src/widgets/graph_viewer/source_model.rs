use readout_core::types::DeviceId;
use std::path::PathBuf;

pub type ViewerSourceId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XDomain {
    WallClock,
    SequenceIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerSourceKind {
    CsvFile { path: PathBuf },
    LiveCsvTail { device: DeviceId, path: PathBuf },
    RuntimeDevice { device: DeviceId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewerSample {
    pub x: f64,
    pub x_label: String,
    pub value: Option<f64>,
    pub device: String,
    pub unit: String,
    pub mode: String,
    pub is_overload: bool,
    pub is_open: bool,
    pub is_short: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Ready,
    Waiting(String),
    Error(String),
}
