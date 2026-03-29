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

        let config_clone = config.clone();
        let ctx_clone = ctx.clone();
        let cancel_clone = cancel.clone();
        let bg_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async move {
                let runtime_cancel = cancel_clone.clone();
                let runtime_handle = tokio::spawn(async move {
                    runtime.run(runtime_cancel).await;
                });

                // Persistence writers
                let mut mm_csv = if config_clone.multimeter_csv_logging_enabled
                    && !config_clone.multimeter_csv_log_file_path.is_empty()
                {
                    let mut logger = readout_persistence::csv_logger::CsvLogger::new(
                        PathBuf::from(&config_clone.multimeter_csv_log_file_path),
                    );
                    logger.start();
                    Some(logger)
                } else {
                    None
                };

                let mut usbc_csv = if config_clone.usbc_csv_logging_enabled
                    && !config_clone.usbc_csv_log_file_path.is_empty()
                {
                    let mut logger = readout_persistence::csv_logger::CsvLogger::new(
                        PathBuf::from(&config_clone.usbc_csv_log_file_path),
                    );
                    logger.start();
                    Some(logger)
                } else {
                    None
                };

                let mut mm_obs = if config_clone.multimeter_obs_enabled
                    && !config_clone.multimeter_output_file.is_empty()
                {
                    let mut writer = readout_persistence::obs_writer::ObsOutputWriter::new(
                        PathBuf::from(&config_clone.multimeter_output_file),
                    );
                    writer.start();
                    Some(writer)
                } else {
                    None
                };

                let mut usbc_obs = if config_clone.usbc_obs_enabled
                    && !config_clone.usbc_output_file.is_empty()
                {
                    let mut writer = readout_persistence::obs_writer::ObsOutputWriter::new(
                        PathBuf::from(&config_clone.usbc_output_file),
                    );
                    writer.start();
                    Some(writer)
                } else {
                    None
                };

                let mut last_repaint = std::time::Instant::now();
                let repaint_interval = std::time::Duration::from_millis(200);
                loop {
                    tokio::select! {
                        _ = cancel_clone.cancelled() => break,
                        result = broadcast_rx.recv() => {
                            match result {
                                Ok(event) => {
                                    // Forward measurements to persistence writers
                                    if let RuntimeEvent::Measurement { ref device, ref value } = event {
                                        match device {
                                            DeviceId::Multimeter => {
                                                if let Some(ref csv) = mm_csv {
                                                    csv.log(value);
                                                }
                                                if let Some(ref obs) = mm_obs {
                                                    obs.update(value);
                                                }
                                            }
                                            DeviceId::UsbC => {
                                                if let Some(ref csv) = usbc_csv {
                                                    csv.log(value);
                                                }
                                                if let Some(ref obs) = usbc_obs {
                                                    obs.update(value);
                                                }
                                            }
                                        }
                                    }
                                    let _ = std_tx.send(event);
                                    let now = std::time::Instant::now();
                                    if now.duration_since(last_repaint) >= repaint_interval {
                                        ctx_clone.request_repaint();
                                        last_repaint = now;
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!("GUI lagged {n} events");
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    }
                }

                // Graceful shutdown of persistence writers
                if let Some(ref mut w) = mm_csv { w.shutdown().await; }
                if let Some(ref mut w) = usbc_csv { w.shutdown().await; }
                if let Some(ref mut w) = mm_obs { w.shutdown().await; }
                if let Some(ref mut w) = usbc_obs { w.shutdown().await; }

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

const WINDOW_WIDTH: f32 = 340.0;
const WINDOW_HEIGHT_BOTH: f32 = 520.0;

pub(crate) fn initial_window_size() -> [f32; 2] {
    [WINDOW_WIDTH, WINDOW_HEIGHT_BOTH]
}

pub struct ReadOutApp {
    runtime: Option<RuntimeHandle>,
    state: DashboardState,
    settings_panel: widgets::settings::SettingsPanel,
    wizard: widgets::first_run_wizard::FirstRunWizard,
    audio: crate::audio::AlarmAudio,
    running: bool,
    always_on_top: bool,
    csv_viewer: widgets::csv_viewer::CsvViewerWindow,
    show_mm: bool,
    show_usbc: bool,
    usbc_metric: UsbCMetric,
    selected_range_idx: usize,
    mm_chart_visible: bool,
    usbc_chart_visible: bool,
    config: AppConfiguration,
    config_path: PathBuf,
    ctx: egui::Context,
    applied_theme: Option<readout_persistence::config::DashboardTheme>,
    meter_control: widgets::meter_control::MeterControlPanel,
    config_save_tx: Option<std::sync::mpsc::Sender<(AppConfiguration, PathBuf)>>,
    config_save_thread: Option<std::thread::JoinHandle<()>>,
    update_check: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
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

        // Single background thread for serialized config saves (prevents race conditions)
        let (config_save_tx, config_save_rx) =
            std::sync::mpsc::channel::<(AppConfiguration, PathBuf)>();
        let config_save_thread = std::thread::spawn(move || {
            while let Ok(mut latest) = config_save_rx.recv() {
                // Drain to latest — only the newest config matters
                while let Ok(newer) = config_save_rx.try_recv() {
                    latest = newer;
                }
                if let Err(e) = config_store::save(&latest.0, &latest.1) {
                    tracing::error!("Failed to save config: {e:?}");
                }
            }
        });

        let state = Self::dashboard_state_from_config(&config);

        // Check for updates in background
        let update_check = if config.check_for_updates {
            let result = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
            let r = result.clone();
            std::thread::spawn(move || {
                if let Some(version) = readout_core::update_checker::check_for_update() {
                    *r.lock().unwrap() = Some(version);
                }
            });
            Some(result)
        } else {
            None
        };

        Self {
            runtime,
            state,
            settings_panel: widgets::settings::SettingsPanel::new(&config),
            wizard: widgets::first_run_wizard::FirstRunWizard::new(&config, first_run),
            audio: crate::audio::AlarmAudio::new(),
            running: !first_run,
            always_on_top: config.always_on_top,
            csv_viewer: widgets::csv_viewer::CsvViewerWindow::new(),
            show_mm: config.show_mm,
            show_usbc: config.show_usbc,
            usbc_metric: UsbCMetric::Voltage,
            selected_range_idx: 0,
            mm_chart_visible: true,
            usbc_chart_visible: true,
            config_path,
            ctx: ctx.clone(),
            applied_theme: None,
            meter_control: widgets::meter_control::MeterControlPanel::new(),
            config_save_tx: Some(config_save_tx),
            config_save_thread: Some(config_save_thread),
            update_check,
            config,
        }
    }

    fn start_runtime(&mut self) {
        if self.runtime.is_some() {
            return;
        }
        self.runtime = Some(RuntimeHandle::start(&self.config, &self.ctx));
        self.running = true;
        self.state = Self::dashboard_state_from_config(&self.config);
    }

    fn save_config_async(&self) {
        self.enqueue_config_save(self.config.clone());
    }

    fn apply_device_recording_action(
        &mut self,
        action: widgets::device_section::DeviceRecordingAction,
    ) {
        let old_config = self.config.clone();
        let changed = match action {
            widgets::device_section::DeviceRecordingAction::ToggleCsvLogging(
                DeviceId::Multimeter,
            ) => {
                self.config.multimeter_csv_logging_enabled =
                    !self.config.multimeter_csv_logging_enabled;
                true
            }
            widgets::device_section::DeviceRecordingAction::ToggleCsvLogging(DeviceId::UsbC) => {
                self.config.usbc_csv_logging_enabled = !self.config.usbc_csv_logging_enabled;
                true
            }
            widgets::device_section::DeviceRecordingAction::ToggleObsOutput(
                DeviceId::Multimeter,
            ) => {
                self.config.multimeter_obs_enabled = !self.config.multimeter_obs_enabled;
                true
            }
            widgets::device_section::DeviceRecordingAction::ToggleObsOutput(DeviceId::UsbC) => {
                self.config.usbc_obs_enabled = !self.config.usbc_obs_enabled;
                true
            }
            widgets::device_section::DeviceRecordingAction::None => false,
        };

        if changed {
            let needs_restart =
                self.runtime.is_some() && runtime_settings_changed(&old_config, &self.config);
            if needs_restart {
                self.restart_runtime();
            } else {
                self.state.log_capture_enabled = self.config.runtime_log_capture_enabled;
            }
            self.enqueue_config_save(self.config.clone());
        }
    }

    fn restart_runtime(&mut self) {
        if let Some(mut rt) = self.runtime.take() {
            rt.shutdown();
        }
        self.runtime = Some(RuntimeHandle::start(&self.config, &self.ctx));
        self.state = Self::dashboard_state_from_config(&self.config);
    }

    fn enqueue_config_save(&self, config: AppConfiguration) {
        if let Some(tx) = &self.config_save_tx {
            let _ = tx.send((config, self.config_path.clone()));
        }
    }

    fn dashboard_state_from_config(config: &AppConfiguration) -> DashboardState {
        let mut state = DashboardState::new();
        state.log_capture_enabled = config.runtime_log_capture_enabled;
        state
    }
}

impl Drop for ReadOutApp {
    fn drop(&mut self) {
        self.config_save_tx.take();
        if let Some(handle) = self.config_save_thread.take() {
            let _ = handle.join();
        }
    }
}

fn runtime_settings_changed(old: &AppConfiguration, new: &AppConfiguration) -> bool {
    let multimeter_obs_path_changed = old.multimeter_output_file != new.multimeter_output_file
        && (old.multimeter_obs_enabled || new.multimeter_obs_enabled);
    let usbc_obs_path_changed = old.usbc_output_file != new.usbc_output_file
        && (old.usbc_obs_enabled || new.usbc_obs_enabled);

    old.multimeter_port != new.multimeter_port
        || old.usbc_port != new.usbc_port
        || old.multimeter_enabled != new.multimeter_enabled
        || old.usbc_enabled != new.usbc_enabled
        || old.use_simulator != new.use_simulator
        || old.sample_rate_hz != new.sample_rate_hz
        || old.short_threshold != new.short_threshold
        || old.dcv_high_alarm_enabled != new.dcv_high_alarm_enabled
        || old.dcv_high_alarm_value != new.dcv_high_alarm_value
        || old.dcv_low_alarm_enabled != new.dcv_low_alarm_enabled
        || old.dcv_low_alarm_value != new.dcv_low_alarm_value
        || old.multimeter_auto_reconnect != new.multimeter_auto_reconnect
        || old.usbc_auto_reconnect != new.usbc_auto_reconnect
        || old.multimeter_csv_logging_enabled != new.multimeter_csv_logging_enabled
        || old.multimeter_csv_log_file_path != new.multimeter_csv_log_file_path
        || old.usbc_csv_logging_enabled != new.usbc_csv_logging_enabled
        || old.usbc_csv_log_file_path != new.usbc_csv_log_file_path
        || old.multimeter_obs_enabled != new.multimeter_obs_enabled
        || old.usbc_obs_enabled != new.usbc_obs_enabled
        || multimeter_obs_path_changed
        || usbc_obs_path_changed
}

impl eframe::App for ReadOutApp {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.applied_theme != Some(self.config.dashboard_theme) {
            crate::theme::apply_theme(ctx, self.config.dashboard_theme);
            self.applied_theme = Some(self.config.dashboard_theme);
        }

        // Drain runtime events
        if let Some(ref runtime) = self.runtime {
            while let Ok(event) = runtime.event_rx.try_recv() {
                self.csv_viewer.handle_runtime_event(&event);
                self.state.handle_event(event);
            }
        }

        // Poll update check result
        if self.state.update_available.is_none()
            && let Some(ref check) = self.update_check
            && let Ok(guard) = check.try_lock()
            && let Some(ref version) = *guard
        {
            self.state.update_available = Some(version.clone());
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
            if i.modifiers.command && i.key_pressed(egui::Key::Comma) {
                self.settings_panel.open = true;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::M) {
                self.meter_control.open = !self.meter_control.open;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::L) {
                self.csv_viewer.open = true;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::K) {
                for pipeline in self.state.chart_pipelines.values_mut() {
                    pipeline.clear();
                }
                for pipeline in self.state.usbc_chart_pipelines.values_mut() {
                    pipeline.clear();
                }
            }
        });

        // Overlays
        if let Some(new_config) = self.wizard.show(ctx) {
            self.config = new_config;
            self.enqueue_config_save(self.config.clone());
            self.start_runtime();
        }

        {
            let old_config = self.config.clone();
            let theme = self.config.dashboard_theme;
            let changed = self.settings_panel.show(
                ctx,
                &mut self.config,
                theme,
                self.always_on_top,
                &self.state.update_available,
            );
            if changed {
                let needs_restart =
                    self.runtime.is_some() && runtime_settings_changed(&old_config, &self.config);
                if needs_restart {
                    self.restart_runtime();
                } else {
                    self.state.log_capture_enabled = self.config.runtime_log_capture_enabled;
                }
                self.enqueue_config_save(self.config.clone());
            }
        }

        // Meter Control viewport
        if self.meter_control.open {
            let connected = matches!(
                self.state.connection_for(DeviceId::Multimeter),
                ConnectionState::Connected
            );
            let command_tx = self.runtime.as_ref().map(|r| r.command_tx.clone());

            let mut close_requested = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("meter_control"),
                {
                    let mut vp = egui::ViewportBuilder::default()
                        .with_title("Multimeter Control")
                        .with_inner_size([340.0, 560.0])
                        .with_resizable(false);
                    if self.always_on_top {
                        vp = vp.with_always_on_top();
                    }
                    vp
                },
                |ctx, _class| {
                    close_requested = ctx.input(|i| i.viewport().close_requested());
                    if self.meter_control.applied_theme != Some(self.config.dashboard_theme) {
                        crate::theme::apply_theme(ctx, self.config.dashboard_theme);
                        self.meter_control.applied_theme = Some(self.config.dashboard_theme);
                    }
                    let mc_action = widgets::meter_control::show(
                        ctx,
                        &self.state,
                        command_tx.as_ref(),
                        connected,
                        self.config.dashboard_beep_master_enabled,
                        self.config.beep_on_short_meter,
                    );
                    match mc_action {
                        widgets::meter_control::MeterControlAction::TogglePcBeep => {
                            self.config.dashboard_beep_master_enabled =
                                !self.config.dashboard_beep_master_enabled;
                            self.save_config_async();
                        }
                        widgets::meter_control::MeterControlAction::ToggleMeterBeep => {
                            self.config.beep_on_short_meter = !self.config.beep_on_short_meter;
                            if let Some(ref runtime) = self.runtime {
                                runtime.meter_beep_flag.store(
                                    self.config.beep_on_short_meter,
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                            }
                            self.save_config_async();
                        }
                        widgets::meter_control::MeterControlAction::None => {}
                    }
                },
            );
            if close_requested {
                self.meter_control.open = false;
            }
        }

        self.csv_viewer.show(ctx, &self.config);

        // --- Main content ---
        let mut toolbar_action = widgets::toolbar::ToolbarAction::None;
        let mut section_action = widgets::device_section::SectionAction::None;
        let mut content_height = 0.0_f32;
        egui::CentralPanel::default()
            .show(ctx, |ui| {
                let content_start = ui.cursor().top();

                // Title bar
                let title_state = widgets::toolbar::TitleBarState {
                    always_on_top: self.always_on_top,
                    selected_range_idx: self.selected_range_idx,
                    show_mm: self.show_mm,
                    show_usbc: self.show_usbc,
                };
                let ta = widgets::toolbar::show_title_bar(ui, &title_state);
                if !matches!(ta, widgets::toolbar::ToolbarAction::None) {
                    toolbar_action = ta;
                }

                ui.separator();

                // Multimeter section
                if self.show_mm {
                    let default_conn = ConnectionState::Disconnected;
                    let mm_conn = self
                        .state
                        .connection_state
                        .get(&DeviceId::Multimeter)
                        .unwrap_or(&default_conn);
                    let mm_alarm = self
                        .state
                        .alarm_state
                        .get(&DeviceId::Multimeter)
                        .copied()
                        .unwrap_or(AlarmState::None);
                    let mm_pipeline = self.state.chart_pipelines.get_mut(&DeviceId::Multimeter);
                    let mm_csv_configured = !self.config.multimeter_csv_log_file_path.is_empty();
                    let mm_csv = self.config.multimeter_csv_logging_enabled && mm_csv_configured;
                    let mm_obs_configured = !self.config.multimeter_output_file.is_empty();
                    let mm_obs = self.config.multimeter_obs_enabled && mm_obs_configured;
                    let (_, ta, recording_action) = widgets::device_section::show(
                        ui,
                        DeviceId::Multimeter,
                        self.state.latest_measurement.get(&DeviceId::Multimeter),
                        mm_conn,
                        mm_alarm,
                        mm_pipeline,
                        self.selected_range_idx,
                        self.usbc_metric,
                        &mut self.mm_chart_visible,
                        mm_csv_configured,
                        mm_csv,
                        mm_obs_configured,
                        mm_obs,
                    );
                    self.apply_device_recording_action(recording_action);
                    if !matches!(ta, widgets::toolbar::ToolbarAction::None) {
                        toolbar_action = ta;
                    }
                }

                if self.show_mm && self.show_usbc {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                }

                // USB-C section
                if self.show_usbc {
                    let default_conn = ConnectionState::Disconnected;
                    let usbc_conn = self
                        .state
                        .connection_state
                        .get(&DeviceId::UsbC)
                        .unwrap_or(&default_conn);
                    let usbc_alarm = self
                        .state
                        .alarm_state
                        .get(&DeviceId::UsbC)
                        .copied()
                        .unwrap_or(AlarmState::None);
                    let usbc_pipeline = self.state.usbc_chart_pipelines.get_mut(&self.usbc_metric);
                    let usbc_csv_configured = !self.config.usbc_csv_log_file_path.is_empty();
                    let usbc_csv = self.config.usbc_csv_logging_enabled && usbc_csv_configured;
                    let usbc_obs_configured = !self.config.usbc_output_file.is_empty();
                    let usbc_obs = self.config.usbc_obs_enabled && usbc_obs_configured;
                    let (sa, _, recording_action) = widgets::device_section::show(
                        ui,
                        DeviceId::UsbC,
                        self.state.latest_measurement.get(&DeviceId::UsbC),
                        usbc_conn,
                        usbc_alarm,
                        usbc_pipeline,
                        self.selected_range_idx,
                        self.usbc_metric,
                        &mut self.usbc_chart_visible,
                        usbc_csv_configured,
                        usbc_csv,
                        usbc_obs_configured,
                        usbc_obs,
                    );
                    self.apply_device_recording_action(recording_action);
                    if !matches!(sa, widgets::device_section::SectionAction::None) {
                        section_action = sa;
                    }
                }

                // Measure content height for dynamic window sizing
                let panel_margin = ui.style().spacing.window_margin;
                content_height = ui.cursor().top() - content_start
                    + panel_margin.top as f32
                    + panel_margin.bottom as f32;
            })
            .response
            .context_menu(|ui| {
                let ta = widgets::toolbar::context_menu(ui, self.state.paused);
                if !matches!(ta, widgets::toolbar::ToolbarAction::None) {
                    toolbar_action = ta;
                }
            });

        // Dynamic window height — adjust to content, keep user's width
        if content_height > 0.0 {
            let current_width = ctx.input(|i| i.viewport_rect().width());
            let max_height = ctx.input(|i| {
                i.viewport_rect()
                    .height()
                    .max(i.viewport().monitor_size.map_or(800.0, |s| s.y - 80.0))
            });
            let target_height = content_height.max(100.0).min(max_height);
            let current_height = ctx.input(|i| i.viewport_rect().height());
            if (current_height - target_height).abs() > 2.0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    current_width,
                    target_height,
                )));
            }
        }

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
            widgets::toolbar::ToolbarAction::SetTimeRange(idx) => {
                self.selected_range_idx = idx;
            }
            widgets::toolbar::ToolbarAction::OpenCsvViewer => {
                self.csv_viewer.open = true;
            }
            widgets::toolbar::ToolbarAction::OpenSettings => {
                self.settings_panel.open = true;
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
            widgets::toolbar::ToolbarAction::OpenMeterControl => {
                self.meter_control.open = true;
            }
            widgets::toolbar::ToolbarAction::ToggleShowMm => {
                if self.show_mm && !self.show_usbc {
                    return;
                }
                self.show_mm = !self.show_mm;
            }
            widgets::toolbar::ToolbarAction::ToggleShowUsbc => {
                if self.show_usbc && !self.show_mm {
                    return;
                }
                self.show_usbc = !self.show_usbc;
            }
            widgets::toolbar::ToolbarAction::None => {}
        }

        // Handle device section actions
        match section_action {
            widgets::device_section::SectionAction::ResetEnergy => {
                if let Some(ref runtime) = self.runtime {
                    let _ = runtime.command_tx.try_send(Command::ResetEnergy {
                        device: DeviceId::UsbC,
                    });
                }
            }
            widgets::device_section::SectionAction::SetUsbcMetric(metric) => {
                self.usbc_metric = metric;
            }
            widgets::device_section::SectionAction::None => {}
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_window_size_dimensions() {
        let [w, h] = initial_window_size();
        assert_eq!(w, WINDOW_WIDTH);
        assert_eq!(h, WINDOW_HEIGHT_BOTH);
    }

    #[test]
    fn runtime_settings_changed_when_obs_enabled_flags_change() {
        let old = AppConfiguration::default();

        let mut multimeter_changed = AppConfiguration::default();
        multimeter_changed.multimeter_obs_enabled = !old.multimeter_obs_enabled;
        assert!(runtime_settings_changed(&old, &multimeter_changed));

        let mut usbc_changed = AppConfiguration::default();
        usbc_changed.usbc_obs_enabled = !old.usbc_obs_enabled;
        assert!(runtime_settings_changed(&old, &usbc_changed));
    }

    #[test]
    fn runtime_settings_changed_ignores_obs_path_changes_when_disabled() {
        let mut old = AppConfiguration::default();
        let mut new = AppConfiguration::default();

        old.multimeter_output_file = "old-mm.txt".to_string();
        new.multimeter_output_file = "new-mm.txt".to_string();
        old.multimeter_obs_enabled = false;
        new.multimeter_obs_enabled = false;
        assert!(!runtime_settings_changed(&old, &new));

        old = AppConfiguration::default();
        new = AppConfiguration::default();
        old.usbc_output_file = "old-usbc.txt".to_string();
        new.usbc_output_file = "new-usbc.txt".to_string();
        old.usbc_obs_enabled = false;
        new.usbc_obs_enabled = false;
        assert!(!runtime_settings_changed(&old, &new));
    }
}
