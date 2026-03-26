use crate::widgets;
use readout_core::dashboard_state::DashboardState;
use readout_core::types::{Command, DeviceId, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use tokio_util::sync::CancellationToken;

pub struct ReadOutApp {
    event_rx: std::sync::mpsc::Receiver<RuntimeEvent>,
    command_tx: tokio::sync::mpsc::Sender<Command>,
    cancel: CancellationToken,
    bg_thread: Option<std::thread::JoinHandle<()>>,
    state: DashboardState,
    chart_state: widgets::chart::ChartState,
    settings_panel: widgets::settings::SettingsPanel,
    running: bool,
    show_log_panel: bool,
    config: AppConfiguration,
}

impl ReadOutApp {
    pub fn new(config: AppConfiguration, ctx: &egui::Context) -> Self {
        let (std_tx, std_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();

        let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
        let command_tx = runtime.command_sender();

        let ctx_clone = ctx.clone();
        let cancel_clone = cancel.clone();
        let bg_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async move {
                let runtime_cancel = cancel_clone.clone();
                let runtime_handle = tokio::spawn(async move {
                    runtime.run(runtime_cancel).await;
                });

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

                let _ = runtime_handle.await;
            });
        });

        Self {
            event_rx: std_rx,
            command_tx,
            cancel,
            bg_thread: Some(bg_thread),
            state: DashboardState::new(),
            chart_state: widgets::chart::ChartState::default(),
            settings_panel: widgets::settings::SettingsPanel::new(&config),
            running: true,
            show_log_panel: true,
            config,
        }
    }

    #[allow(dead_code)]
    pub fn command_sender(&self) -> tokio::sync::mpsc::Sender<Command> {
        self.command_tx.clone()
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Ctrl+P / Cmd+P: toggle pause
            if i.modifiers.command && i.key_pressed(egui::Key::P) {
                self.state.paused = !self.state.paused;
            }
            // Ctrl+L / Cmd+L: toggle log panel
            if i.modifiers.command && i.key_pressed(egui::Key::L) {
                self.show_log_panel = !self.show_log_panel;
            }
            // Ctrl+, / Cmd+,: open settings
            if i.modifiers.command && i.key_pressed(egui::Key::Comma) {
                self.settings_panel.open_with(&self.config);
            }
        });
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
        // Drain events
        while let Ok(event) = self.event_rx.try_recv() {
            self.state.handle_event(event);
        }

        self.handle_keyboard_shortcuts(ctx);

        // Settings window (floating)
        if let Some(new_config) = self.settings_panel.show(ctx) {
            self.config = new_config;
            // TODO: send Command::UpdateConfig when implemented
        }

        // Periodic repaint for status updates
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        // --- Header ---
        let mut paused = self.state.paused;
        let mut header_action = widgets::header::HeaderAction::None;
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            header_action = widgets::header::show(ui, &self.state, self.running, &mut paused);
        });
        self.state.paused = paused;

        match header_action {
            widgets::header::HeaderAction::Stop => {
                if let Err(e) = self.command_tx.try_send(Command::Stop) {
                    tracing::warn!("Failed to send Stop command: {e}");
                }
                self.running = false;
            }
            widgets::header::HeaderAction::Start => {
                // Restart not yet implemented — runtime would need to be recreated.
                // For now Start is disabled in the header when running=false.
            }
            widgets::header::HeaderAction::None => {}
        }

        // --- Status strip ---
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            widgets::status_strip::show(ui, &self.state);
        });

        // --- Log panel (collapsible) ---
        if self.show_log_panel {
            egui::TopBottomPanel::bottom("log_panel")
                .resizable(true)
                .min_height(60.0)
                .default_height(150.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Log");
                        if ui.small_button("✕").clicked() {
                            self.show_log_panel = false;
                        }
                    });
                    ui.separator();
                    widgets::log_panel::show(ui, &self.state);
                });
        }

        // --- Central: device cards ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                widgets::device_card::show(
                    &mut cols[0],
                    DeviceId::Multimeter,
                    self.state.latest_measurement.get(&DeviceId::Multimeter),
                    self.state.alarm_for(DeviceId::Multimeter),
                );
                widgets::device_card::show(
                    &mut cols[1],
                    DeviceId::UsbC,
                    self.state.latest_measurement.get(&DeviceId::UsbC),
                    self.state.alarm_for(DeviceId::UsbC),
                );
            });

            ui.separator();
            widgets::chart::show(ui, &mut self.state.chart_pipelines, &mut self.chart_state);
        });
    }
}
