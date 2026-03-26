use readout_core::types::DeviceMeasurement;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub struct ObsOutputWriter {
    path: PathBuf,
    tx: mpsc::Sender<String>,
    rx: Option<mpsc::Receiver<String>>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ObsOutputWriter {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel(4);
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

    pub fn update(&self, measurement: &DeviceMeasurement) {
        let text = measurement
            .primary_value
            .map(|v| format!("{v} {}", measurement.primary_unit))
            .unwrap_or_else(|| format!("OL {}", measurement.primary_unit));
        // Best-effort: skip if channel full (only latest value matters)
        let _ = self.tx.try_send(text);
    }

    async fn writer_task(path: PathBuf, mut rx: mpsc::Receiver<String>) {
        let mut tick = interval(Duration::from_millis(500)); // ~2 Hz
        let mut latest: Option<String> = None;

        loop {
            tokio::select! {
                val = rx.recv() => {
                    match val {
                        Some(text) => latest = Some(text),
                        None => break,
                    }
                }
                _ = tick.tick() => {
                    if let Some(ref text) = latest {
                        let _ = std::fs::write(&path, text);
                    }
                }
            }
        }
        // Final write on shutdown
        if let Some(ref text) = latest {
            let _ = std::fs::write(&path, text);
        }
    }
}

impl Drop for ObsOutputWriter {
    fn drop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }
}
