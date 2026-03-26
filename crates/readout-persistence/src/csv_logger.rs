use readout_core::types::{DeviceId, DeviceMeasurement};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

const CSV_HEADER: &str = "timestamp,device,value,unit,mode,is_overload,is_open,is_short";

enum CsvMessage {
    Row(CsvRow),
    Flush(oneshot::Sender<()>),
}

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
    tx: mpsc::Sender<CsvMessage>,
    rx: Option<mpsc::Receiver<CsvMessage>>,
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
        let _ = self.tx.try_send(CsvMessage::Row(row));
    }

    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(CsvMessage::Flush(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }

    async fn writer_task(path: PathBuf, mut rx: mpsc::Receiver<CsvMessage>) {
        use std::io::{BufWriter, Write};

        let file = match tokio::task::spawn_blocking({
            let path = path.clone();
            move || {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
            }
        })
        .await
        {
            Ok(Ok(f)) => f,
            _ => {
                tracing::error!("Failed to open CSV file: {:?}", path);
                return;
            }
        };

        let mut writer = BufWriter::new(file);

        // Write header if file is empty
        if writer
            .get_ref()
            .metadata()
            .map(|m| m.len() == 0)
            .unwrap_or(true)
        {
            let _ = writeln!(writer, "{CSV_HEADER}");
            let _ = writer.flush();
        }

        while let Some(msg) = rx.recv().await {
            match msg {
                CsvMessage::Row(row) => {
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
                    let _ = writeln!(writer, "{line}");
                }
                CsvMessage::Flush(done) => {
                    let _ = writer.flush();
                    let _ = done.send(());
                }
            }
        }
        // Drain complete — flush remaining buffered data
        let _ = writer.flush();
    }
}

// When CsvLogger is dropped, self.tx is dropped automatically, closing the channel.
// The writer task will drain remaining messages via recv() returning None, then flush.
