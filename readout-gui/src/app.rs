use readout_core::types::{Command, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use tokio_util::sync::CancellationToken;

pub struct ReadOutApp {
    event_rx: std::sync::mpsc::Receiver<RuntimeEvent>,
    command_tx: tokio::sync::mpsc::Sender<Command>,
    cancel: CancellationToken,
    bg_thread: Option<std::thread::JoinHandle<()>>,
    config: AppConfiguration,
}

impl ReadOutApp {
    pub fn new(config: AppConfiguration, ctx: &egui::Context) -> Self {
        let (std_tx, std_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();

        let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
        let command_tx = runtime.command_sender();

        // Bridge: tokio broadcast → std::sync::mpsc → egui
        let ctx_clone = ctx.clone();
        let cancel_clone = cancel.clone();
        let bg_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async move {
                let runtime_cancel = cancel_clone.clone();
                let runtime_handle = tokio::spawn(async move {
                    runtime.run(runtime_cancel).await;
                });

                // Forward events until cancelled or channel closed
                loop {
                    tokio::select! {
                        _ = cancel_clone.cancelled() => break,
                        result = broadcast_rx.recv() => {
                            match result {
                                Ok(event) => {
                                    let _ = std_tx.send(event);
                                    ctx_clone.request_repaint();
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!("GUI lagged {n} events");
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    }
                }

                // Wait for runtime graceful shutdown before tokio runtime drops
                let _ = runtime_handle.await;
            });
        });

        Self {
            event_rx: std_rx,
            command_tx,
            cancel,
            bg_thread: Some(bg_thread),
            config,
        }
    }

    #[allow(dead_code)]
    pub fn command_sender(&self) -> tokio::sync::mpsc::Sender<Command> {
        self.command_tx.clone()
    }
}

impl Drop for ReadOutApp {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.bg_thread.take() {
            let _ = handle.join();
        }
    }
}

impl eframe::App for ReadOutApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(_event) = self.event_rx.try_recv() {
            // TODO: dispatch to DashboardState in Task 17
        }

        // Repaint at ~4 Hz for status updates; data-driven repaints come from bridge thread
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("readout-gui");
            ui.label(format!(
                "Simulator: {} | Multimeter: {} | USB-C: {}",
                self.config.use_simulator,
                self.config.multimeter_enabled,
                self.config.usbc_enabled,
            ));
            ui.label("Dashboard coming in next task...");
        });
    }
}
