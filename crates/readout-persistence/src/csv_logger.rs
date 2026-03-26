use readout_core::types::{DeviceId, DeviceMeasurement};
use std::path::PathBuf;
use tokio::sync::mpsc;

const CSV_HEADER: &str = "timestamp,device,value,unit,mode,is_overload,is_open,is_short";

struct CsvRow {
    timestamp: String,
    device: String,
    value: String,
    unit: String,
    mode: String,
    is_overload: bool,
    is_open: bool,
    is_short: bool,
}

pub struct CsvLogger {
    path: PathBuf,
    tx: mpsc::Sender<CsvRow>,
    rx: Option<mpsc::Receiver<CsvRow>>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl CsvLogger {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel(256);
        Self {
            path,
            tx,
            rx: Some(rx),
            task_handle: None,
        }
    }

    pub fn start(&mut self) {
        let Some(rx) = self.rx.take() else {
            return;
        };
        let path = self.path.clone();
        self.task_handle = Some(tokio::spawn(Self::writer_task(path, rx)));
    }

    pub fn log(&self, measurement: &DeviceMeasurement) {
        let row = CsvRow {
            timestamp: chrono::Utc::now().to_rfc3339(),
            device: match measurement.device {
                DeviceId::Multimeter => "Multimeter".into(),
                DeviceId::UsbC => "UsbC".into(),
            },
            value: measurement
                .primary_value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "OL".into()),
            unit: measurement.primary_unit.clone(),
            mode: measurement.mode_string.clone(),
            is_overload: measurement.is_overload,
            is_open: measurement.is_open,
            is_short: measurement.is_short,
        };
        // Best-effort: drop if channel full
        let _ = self.tx.try_send(row);
    }

    pub async fn flush(&self) {
        // Give the writer task time to process pending rows
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    async fn writer_task(path: PathBuf, mut rx: mpsc::Receiver<CsvRow>) {
        use std::io::Write;

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path);

        let Ok(mut file) = file else {
            tracing::error!("Failed to open CSV file: {:?}", path);
            return;
        };

        // Write header if file is empty
        if file.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            let _ = writeln!(file, "{CSV_HEADER}");
        }

        while let Some(row) = rx.recv().await {
            let line = format!(
                "{},{},{},{},{},{},{},{}",
                row.timestamp,
                row.device,
                row.value,
                row.unit,
                row.mode,
                row.is_overload,
                row.is_open,
                row.is_short,
            );
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }
}

impl Drop for CsvLogger {
    fn drop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }
}
