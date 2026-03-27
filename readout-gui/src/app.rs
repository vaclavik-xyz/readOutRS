use crate::widgets;
use readout_core::dashboard_state::DashboardState;
use readout_core::types::{Command, DeviceId, RuntimeEvent};
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
    chart_state: widgets::chart::ChartState,
    settings_panel: widgets::settings::SettingsPanel,
    wizard: widgets::first_run_wizard::FirstRunWizard,
    popout_state: crate::popout::PopoutState,
    audio: crate::audio::AlarmAudio,
    running: bool,
    show_log_panel: bool,
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
        // Defer runtime start if first-run wizard is active
        let runtime = if first_run {
            None
        } else {
            Some(RuntimeHandle::start(&config, ctx))
        };

        Self {
            runtime,
            state: DashboardState::new(),
            chart_state: widgets::chart::ChartState::default(),
            settings_panel: widgets::settings::SettingsPanel::new(&config),
            wizard: widgets::first_run_wizard::FirstRunWizard::new(&config, first_run),
            popout_state: crate::popout::PopoutState {
                open: config.popout_open,
                show_mm: config.popout_show_mm,
                show_usbc: config.popout_show_usbc,
            },
            audio: crate::audio::AlarmAudio::new(),
            running: !first_run,
            show_log_panel: true,
            config,
            config_path,
            ctx: ctx.clone(),
            applied_theme: None,
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

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::P) {
                self.state.paused = !self.state.paused;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::L) {
                self.show_log_panel = !self.show_log_panel;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Comma) {
                self.settings_panel.open_with(&self.config);
            }
            // Ctrl+1 / Cmd+1: toggle popout
            if i.modifiers.command && i.key_pressed(egui::Key::Num1) {
                self.popout_state.open = !self.popout_state.open;
            }
        });
    }
}

impl eframe::App for ReadOutApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme on change
        if self.applied_theme != Some(self.config.dashboard_theme) {
            crate::theme::apply_theme(ctx, self.config.dashboard_theme);
            self.applied_theme = Some(self.config.dashboard_theme);
        }

        // Drain events from runtime
        if let Some(ref runtime) = self.runtime {
            while let Ok(event) = runtime.event_rx.try_recv() {
                self.state.handle_event(event);
            }
        }

        // Continuous alarm tone — on while alarm is active, off when cleared
        {
            use readout_core::types::AlarmState;
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

        self.handle_keyboard_shortcuts(ctx);

        // Combined popout window
        if self.popout_state.open {
            let popout_action = {
                use std::time::Duration;

                let (range, _) =
                    crate::widgets::chart::RANGE_OPTIONS[self.chart_state.selected_range_idx];
                let target_points = 200;

                let now = self
                    .state
                    .chart_pipelines
                    .values()
                    .filter_map(|p| p.latest_timestamp())
                    .chain(
                        self.state
                            .usbc_chart_pipelines
                            .values()
                            .filter_map(|p| p.latest_timestamp()),
                    )
                    .max()
                    .unwrap_or(Duration::ZERO);

                let mm_chart_data: Vec<[f64; 2]> = self
                    .state
                    .chart_pipelines
                    .get_mut(&DeviceId::Multimeter)
                    .map(|p| {
                        p.query_with_now(range, target_points, now)
                            .iter()
                            .map(|(t, v)| [t.as_secs_f64(), *v])
                            .collect()
                    })
                    .unwrap_or_default();

                let usbc_chart_data: Vec<[f64; 2]> = self
                    .state
                    .usbc_chart_pipelines
                    .get_mut(&self.chart_state.usbc_metric)
                    .map(|p| {
                        p.query_with_now(range, target_points, now)
                            .iter()
                            .map(|(t, v)| [t.as_secs_f64(), *v])
                            .collect()
                    })
                    .unwrap_or_default();

                let input = crate::popout::PopoutInput {
                    mm_measurement: self
                        .state
                        .latest_measurement
                        .get(&DeviceId::Multimeter)
                        .cloned(),
                    usbc_measurement: self
                        .state
                        .latest_measurement
                        .get(&DeviceId::UsbC)
                        .cloned(),
                    mm_connection: self.state.connection_for(DeviceId::Multimeter).clone(),
                    usbc_connection: self.state.connection_for(DeviceId::UsbC).clone(),
                    mm_alarm: self.state.alarm_for(DeviceId::Multimeter),
                    usbc_alarm: self.state.alarm_for(DeviceId::UsbC),
                    mm_chart_data,
                    usbc_chart_data,
                    paused: self.state.paused,
                    pc_beep_enabled: self.config.dashboard_beep_master_enabled,
                    meter_beep_enabled: self.config.beep_on_short_meter,
                    usbc_metric: self.chart_state.usbc_metric,
                    selected_range_idx: self.chart_state.selected_range_idx,
                };

                crate::popout::show_combined_popout(ctx, &mut self.popout_state, input)
            };

            match popout_action {
                crate::popout::PopoutAction::TogglePause => {
                    self.state.paused = !self.state.paused;
                }
                crate::popout::PopoutAction::TogglePcBeep => {
                    self.config.dashboard_beep_master_enabled =
                        !self.config.dashboard_beep_master_enabled;
                    let path = self.config_path.clone();
                    let config = self.config.clone();
                    std::thread::spawn(move || {
                        let _ = readout_persistence::config_store::save(&config, &path);
                    });
                }
                crate::popout::PopoutAction::ToggleMeterBeep => {
                    self.config.beep_on_short_meter = !self.config.beep_on_short_meter;
                    if let Some(ref runtime) = self.runtime {
                        runtime.meter_beep_flag.store(
                            self.config.beep_on_short_meter,
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    }
                    let path = self.config_path.clone();
                    let config = self.config.clone();
                    std::thread::spawn(move || {
                        let _ = readout_persistence::config_store::save(&config, &path);
                    });
                }
                crate::popout::PopoutAction::ResetEnergy => {
                    if let Some(ref runtime) = self.runtime {
                        let _ = runtime
                            .command_tx
                            .try_send(Command::ResetEnergy { device: DeviceId::UsbC });
                    }
                }
                crate::popout::PopoutAction::SetUsbcMetric(metric) => {
                    self.chart_state.usbc_metric = metric;
                }
                crate::popout::PopoutAction::SetTimeRange(idx) => {
                    self.chart_state.selected_range_idx = idx;
                }
                crate::popout::PopoutAction::None => {}
            }
        }

        // First-run wizard — starts runtime when user finishes
        if let Some(new_config) = self.wizard.show(ctx) {
            if let Err(e) = config_store::save(&new_config, &self.config_path) {
                tracing::error!("Failed to save wizard config: {e:?}");
            }
            self.config = new_config;
            self.start_runtime();
        }

        // Settings window
        if let Some(new_config) = self.settings_panel.show(ctx) {
            if let Err(e) = config_store::save(&new_config, &self.config_path) {
                tracing::error!("Failed to save config: {e:?}");
            }
            self.config = new_config;
        }

        // Sync popout state to config for persistence
        if self.config.popout_open != self.popout_state.open
            || self.config.popout_show_mm != self.popout_state.show_mm
            || self.config.popout_show_usbc != self.popout_state.show_usbc
        {
            self.config.popout_open = self.popout_state.open;
            self.config.popout_show_mm = self.popout_state.show_mm;
            self.config.popout_show_usbc = self.popout_state.show_usbc;
            let path = self.config_path.clone();
            let config = self.config.clone();
            std::thread::spawn(move || {
                let _ = readout_persistence::config_store::save(&config, &path);
            });
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        // --- Header ---
        let mut paused = self.state.paused;
        let mut header_action = widgets::header::HeaderAction::None;
        let header_state = widgets::header::HeaderState {
            pc_beep_enabled: self.config.dashboard_beep_master_enabled,
            meter_beep_enabled: self.config.beep_on_short_meter,
            log_visible: self.show_log_panel,
            popout_open: self.popout_state.open,
        };
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            header_action = widgets::header::show(ui, &self.state, self.running, &mut paused, &header_state);
        });
        self.state.paused = paused;

        match header_action {
            widgets::header::HeaderAction::Stop => {
                if let Some(ref runtime) = self.runtime {
                    if let Err(e) = runtime.command_tx.try_send(Command::Stop) {
                        tracing::warn!("Failed to send Stop command: {e}");
                    }
                }
                self.running = false;
            }
            widgets::header::HeaderAction::OpenSettings => {
                self.settings_panel.open_with(&self.config);
            }
            widgets::header::HeaderAction::TogglePcBeep => {
                self.config.dashboard_beep_master_enabled = !self.config.dashboard_beep_master_enabled;
                let path = self.config_path.clone();
                let config = self.config.clone();
                std::thread::spawn(move || {
                    let _ = readout_persistence::config_store::save(&config, &path);
                });
            }
            widgets::header::HeaderAction::ToggleMeterBeep => {
                self.config.beep_on_short_meter = !self.config.beep_on_short_meter;
                // Live-toggle via shared flag — driver sends SCPI immediately
                if let Some(ref runtime) = self.runtime {
                    runtime.meter_beep_flag.store(
                        self.config.beep_on_short_meter,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                let path = self.config_path.clone();
                let config = self.config.clone();
                std::thread::spawn(move || {
                    let _ = readout_persistence::config_store::save(&config, &path);
                });
            }
            widgets::header::HeaderAction::ToggleLog => {
                self.show_log_panel = !self.show_log_panel;
            }
            widgets::header::HeaderAction::TogglePopout => {
                self.popout_state.open = !self.popout_state.open;
            }
            widgets::header::HeaderAction::None => {}
        }

        // --- Status strip ---
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            widgets::status_strip::show(ui, &self.state, self.config.use_simulator);
        });

        // --- Log panel ---
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

        // --- Central: device cards + chart ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                widgets::device_card::show(
                    &mut cols[0],
                    DeviceId::Multimeter,
                    self.state.latest_measurement.get(&DeviceId::Multimeter),
                    self.state.alarm_for(DeviceId::Multimeter),
                    self.state.connection_for(DeviceId::Multimeter),
                );
                widgets::device_card::show(
                    &mut cols[1],
                    DeviceId::UsbC,
                    self.state.latest_measurement.get(&DeviceId::UsbC),
                    self.state.alarm_for(DeviceId::UsbC),
                    self.state.connection_for(DeviceId::UsbC),
                );
            });

            ui.add_space(6.0);
            widgets::chart::show(ui, &mut self.state.chart_pipelines, &mut self.state.usbc_chart_pipelines, &mut self.chart_state);
        });
    }
}
