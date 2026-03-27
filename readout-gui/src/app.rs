use crate::widgets;
use readout_core::dashboard_state::{DashboardState, UsbCMetric};
use readout_core::types::{AlarmState, Command, ConnectionState, DeviceId, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use readout_persistence::config_store;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

struct RuntimeHandle {
    event_rx: std::sync::mpsc::Receiver<RuntimeEvent>,
    command_tx: tokio::sync::mpsc::Sender<Command>,
    cancel: CancellationToken,
    bg_thread: Option<std::thread::JoinHandle<()>>,
    meter_beep_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl RuntimeHandle {
    fn start(config: &AppConfiguration, ctx: &egui::Context) -> Self {
        let (std_tx, std_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();

        let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
        let command_tx = runtime.command_sender();
        let meter_beep_flag = runtime.meter_beep_flag();

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
            meter_beep_flag,
        }
    }

    fn shutdown(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.bg_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct ReadOutApp {
    runtime: Option<RuntimeHandle>,
    state: DashboardState,
    settings_panel: widgets::settings::SettingsPanel,
    wizard: widgets::first_run_wizard::FirstRunWizard,
    audio: crate::audio::AlarmAudio,
    running: bool,
    show_mm: bool,
    show_usbc: bool,
    show_log: bool,
    always_on_top: bool,
    usbc_metric: UsbCMetric,
    selected_range_idx: usize,
    config: AppConfiguration,
    config_path: PathBuf,
    ctx: egui::Context,
    applied_theme: Option<readout_persistence::config::DashboardTheme>,
}

impl ReadOutApp {
    pub fn new(
        config: AppConfiguration,
        config_path: PathBuf,
        first_run: bool,
        ctx: &egui::Context,
    ) -> Self {
        let runtime = if first_run {
            None
        } else {
            Some(RuntimeHandle::start(&config, ctx))
        };

        Self {
            runtime,
            state: DashboardState::new(),
            settings_panel: widgets::settings::SettingsPanel::new(&config),
            wizard: widgets::first_run_wizard::FirstRunWizard::new(&config, first_run),
            audio: crate::audio::AlarmAudio::new(),
            running: !first_run,
            show_mm: config.show_mm,
            show_usbc: config.show_usbc,
            show_log: false,
            always_on_top: config.always_on_top,
            usbc_metric: UsbCMetric::Voltage,
            selected_range_idx: 0,
            config_path,
            ctx: ctx.clone(),
            applied_theme: None,
            config,
        }
    }

    fn start_runtime(&mut self) {
        if self.runtime.is_some() {
            return;
        }
        self.runtime = Some(RuntimeHandle::start(&self.config, &self.ctx));
        self.running = true;
        self.state = DashboardState::new();
    }

    fn save_config_async(&self) {
        let path = self.config_path.clone();
        let config = self.config.clone();
        std::thread::spawn(move || {
            let _ = config_store::save(&config, &path);
        });
    }
}

impl eframe::App for ReadOutApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.applied_theme != Some(self.config.dashboard_theme) {
            crate::theme::apply_theme(ctx, self.config.dashboard_theme);
            self.applied_theme = Some(self.config.dashboard_theme);
        }

        // Drain runtime events
        if let Some(ref runtime) = self.runtime {
            while let Ok(event) = runtime.event_rx.try_recv() {
                self.state.handle_event(event);
            }
        }

        // Alarm audio
        {
            let mm_alarm = self.state.alarm_for(DeviceId::Multimeter);
            let should_sound = self.config.dashboard_beep_master_enabled
                && match mm_alarm {
                    AlarmState::Short => self.config.beep_on_short_pc,
                    AlarmState::HighAlarm | AlarmState::LowAlarm => self.config.beep_on_alarm,
                    _ => false,
                };
            self.audio.set_volume(self.config.pc_beep_volume as f32);
            self.audio.set_active(should_sound);
        }

        // Keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::P) {
                self.state.paused = !self.state.paused;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::L) {
                self.show_log = !self.show_log;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Comma) {
                self.settings_panel.open_with(&self.config);
            }
        });

        // Overlays
        if let Some(new_config) = self.wizard.show(ctx) {
            if let Err(e) = config_store::save(&new_config, &self.config_path) {
                tracing::error!("Failed to save wizard config: {e:?}");
            }
            self.config = new_config;
            self.start_runtime();
        }

        if let Some(new_config) = self.settings_panel.show(ctx) {
            if let Err(e) = config_store::save(&new_config, &self.config_path) {
                tracing::error!("Failed to save config: {e:?}");
            }
            self.config = new_config;
        }

        widgets::log_overlay::show(ctx, &self.state, &mut self.show_log);

        // --- Main content ---
        let mut toolbar_action = widgets::toolbar::ToolbarAction::None;
        let mut section_action = widgets::device_section::SectionAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut toolbar_state = widgets::toolbar::ToolbarState {
                show_mm: self.show_mm,
                show_usbc: self.show_usbc,
                paused: self.state.paused,
                pc_beep_enabled: self.config.dashboard_beep_master_enabled,
                meter_beep_enabled: self.config.beep_on_short_meter,
                selected_range_idx: self.selected_range_idx,
                show_log: self.show_log,
                always_on_top: self.always_on_top,
            };

            toolbar_action = widgets::toolbar::show(ui, &mut toolbar_state);

            // Read back visibility changes (toolbar mutates directly)
            self.show_mm = toolbar_state.show_mm;
            self.show_usbc = toolbar_state.show_usbc;

            ui.separator();

            // Use direct field access (not connection_for/alarm_for methods)
            // to avoid borrowing all of DashboardState while chart pipelines are mut-borrowed.
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.show_mm {
                    let default_conn = ConnectionState::Disconnected;
                    let mm_conn = self.state.connection_state
                        .get(&DeviceId::Multimeter)
                        .unwrap_or(&default_conn);
                    let mm_alarm = self.state.alarm_state
                        .get(&DeviceId::Multimeter)
                        .copied()
                        .unwrap_or(AlarmState::None);
                    let mm_pipeline = self.state.chart_pipelines.get_mut(&DeviceId::Multimeter);
                    widgets::device_section::show(
                        ui,
                        DeviceId::Multimeter,
                        self.state.latest_measurement.get(&DeviceId::Multimeter),
                        mm_conn,
                        mm_alarm,
                        mm_pipeline,
                        self.selected_range_idx,
                        self.usbc_metric,
                    );
                }

                if self.show_mm && self.show_usbc {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                }

                if self.show_usbc {
                    let default_conn = ConnectionState::Disconnected;
                    let usbc_conn = self.state.connection_state
                        .get(&DeviceId::UsbC)
                        .unwrap_or(&default_conn);
                    let usbc_alarm = self.state.alarm_state
                        .get(&DeviceId::UsbC)
                        .copied()
                        .unwrap_or(AlarmState::None);
                    let usbc_pipeline =
                        self.state.usbc_chart_pipelines.get_mut(&self.usbc_metric);
                    let sa = widgets::device_section::show(
                        ui,
                        DeviceId::UsbC,
                        self.state.latest_measurement.get(&DeviceId::UsbC),
                        usbc_conn,
                        usbc_alarm,
                        usbc_pipeline,
                        self.selected_range_idx,
                        self.usbc_metric,
                    );
                    if !matches!(sa, widgets::device_section::SectionAction::None) {
                        section_action = sa;
                    }
                }
            });
        });

        // Handle toolbar actions
        match toolbar_action {
            widgets::toolbar::ToolbarAction::TogglePause => {
                self.state.paused = !self.state.paused;
            }
            widgets::toolbar::ToolbarAction::ClearCharts => {
                for pipeline in self.state.chart_pipelines.values_mut() {
                    pipeline.clear();
                }
                for pipeline in self.state.usbc_chart_pipelines.values_mut() {
                    pipeline.clear();
                }
            }
            widgets::toolbar::ToolbarAction::TogglePcBeep => {
                self.config.dashboard_beep_master_enabled =
                    !self.config.dashboard_beep_master_enabled;
                self.save_config_async();
            }
            widgets::toolbar::ToolbarAction::ToggleMeterBeep => {
                self.config.beep_on_short_meter = !self.config.beep_on_short_meter;
                if let Some(ref runtime) = self.runtime {
                    runtime.meter_beep_flag.store(
                        self.config.beep_on_short_meter,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                self.save_config_async();
            }
            widgets::toolbar::ToolbarAction::SetTimeRange(idx) => {
                self.selected_range_idx = idx;
            }
            widgets::toolbar::ToolbarAction::OpenSettings => {
                self.settings_panel.open_with(&self.config);
            }
            widgets::toolbar::ToolbarAction::ToggleLog => {
                self.show_log = !self.show_log;
            }
            widgets::toolbar::ToolbarAction::ToggleAlwaysOnTop => {
                self.always_on_top = !self.always_on_top;
                self.config.always_on_top = self.always_on_top;
                let level = if self.always_on_top {
                    egui::viewport::WindowLevel::AlwaysOnTop
                } else {
                    egui::viewport::WindowLevel::Normal
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                self.save_config_async();
            }
            widgets::toolbar::ToolbarAction::None => {}
        }

        // Handle device section actions
        match section_action {
            widgets::device_section::SectionAction::ResetEnergy => {
                if let Some(ref runtime) = self.runtime {
                    let _ = runtime
                        .command_tx
                        .try_send(Command::ResetEnergy { device: DeviceId::UsbC });
                }
            }
            widgets::device_section::SectionAction::SetUsbcMetric(metric) => {
                self.usbc_metric = metric;
            }
            widgets::device_section::SectionAction::None => {}
        }

        // Persist visibility on change
        if self.config.show_mm != self.show_mm || self.config.show_usbc != self.show_usbc {
            self.config.show_mm = self.show_mm;
            self.config.show_usbc = self.show_usbc;
            self.save_config_async();
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}
