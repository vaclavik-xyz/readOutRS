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

                let mut last_repaint = std::time::Instant::now();
                let repaint_interval = std::time::Duration::from_millis(200);
                loop {
                    tokio::select! {
                        _ = cancel_clone.cancelled() => break,
                        result = broadcast_rx.recv() => {
                            match result {
                                Ok(event) => {
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

const WINDOW_WIDTH: f32 = 320.0;
const WINDOW_HEIGHT_MM: f32 = 238.0;
const WINDOW_HEIGHT_MM_ALARM: f32 = 258.0;
const WINDOW_HEIGHT_USBC: f32 = 329.0;
const WINDOW_HEIGHT_BOTH: f32 = 524.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompactWindowMode {
    Multimeter,
    MultimeterAlarm,
    UsbC,
    Both,
}

impl CompactWindowMode {
    fn from_state(
        show_mm: bool,
        show_usbc: bool,
        mm_alarm: AlarmState,
        _usbc_alarm: AlarmState,
    ) -> Self {
        match (show_mm, show_usbc) {
            (true, false) if mm_alarm != AlarmState::None => Self::MultimeterAlarm,
            (true, false) => Self::Multimeter,
            // USB-C and combined mode use the alarm-capable height to avoid resize churn.
            (false, true) => Self::UsbC,
            (true, true) => Self::Both,
            (false, false) => Self::Multimeter,
        }
    }

    fn height(self) -> f32 {
        match self {
            Self::Multimeter => WINDOW_HEIGHT_MM,
            Self::MultimeterAlarm => WINDOW_HEIGHT_MM_ALARM,
            Self::UsbC => WINDOW_HEIGHT_USBC,
            Self::Both => WINDOW_HEIGHT_BOTH,
        }
    }

    fn inner_size(self) -> egui::Vec2 {
        egui::vec2(WINDOW_WIDTH, self.height())
    }
}

pub(crate) fn initial_window_size(show_mm: bool, show_usbc: bool) -> [f32; 2] {
    let mode = CompactWindowMode::from_state(show_mm, show_usbc, AlarmState::None, AlarmState::None);
    [WINDOW_WIDTH, mode.height()]
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
    window_mode: CompactWindowMode,
    config_save_tx: std::sync::mpsc::Sender<(AppConfiguration, PathBuf)>,
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
        std::thread::spawn(move || {
            while let Ok(mut latest) = config_save_rx.recv() {
                // Drain to latest — only the newest config matters
                while let Ok(newer) = config_save_rx.try_recv() {
                    latest = newer;
                }
                let _ = config_store::save(&latest.0, &latest.1);
            }
        });

        Self {
            runtime,
            state: DashboardState::new(),
            settings_panel: widgets::settings::SettingsPanel::new(&config),
            wizard: widgets::first_run_wizard::FirstRunWizard::new(&config, first_run),
            audio: crate::audio::AlarmAudio::new(),
            running: !first_run,
            show_mm: config.show_mm,
            show_usbc: config.show_usbc,
            show_log: config.runtime_log_panel_visible,
            always_on_top: config.always_on_top,
            usbc_metric: UsbCMetric::Voltage,
            selected_range_idx: 0,
            config_path,
            ctx: ctx.clone(),
            applied_theme: None,
            window_mode: CompactWindowMode::from_state(
                config.show_mm,
                config.show_usbc,
                AlarmState::None,
                AlarmState::None,
            ),
            config_save_tx,
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
        let _ = self.config_save_tx.send((self.config.clone(), self.config_path.clone()));
    }

    fn restart_runtime(&mut self) {
        if let Some(mut rt) = self.runtime.take() {
            rt.shutdown();
        }
        self.runtime = Some(RuntimeHandle::start(&self.config, &self.ctx));
        self.state = DashboardState::new();
    }
}

fn runtime_settings_changed(old: &AppConfiguration, new: &AppConfiguration) -> bool {
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
            let needs_restart = self.runtime.is_some()
                && runtime_settings_changed(&self.config, &new_config);
            self.config = new_config;

            // Sync visibility from dashboard_device_visibility setting
            match self.config.dashboard_device_visibility {
                readout_persistence::config::DashboardDeviceVisibility::Both => {
                    self.show_mm = true;
                    self.show_usbc = true;
                }
                readout_persistence::config::DashboardDeviceVisibility::Multimeter => {
                    self.show_mm = true;
                    self.show_usbc = false;
                }
                readout_persistence::config::DashboardDeviceVisibility::UsbC => {
                    self.show_mm = false;
                    self.show_usbc = true;
                }
            }
            self.config.show_mm = self.show_mm;
            self.config.show_usbc = self.show_usbc;

            // Apply log capture preference
            self.state.log_capture_enabled = self.config.runtime_log_capture_enabled;

            if needs_restart {
                self.restart_runtime();
            }
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

            // Device sections — no ScrollArea so we can measure true content height.
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

        let next_window_mode = CompactWindowMode::from_state(
            self.show_mm,
            self.show_usbc,
            self.state.alarm_for(DeviceId::Multimeter),
            self.state.alarm_for(DeviceId::UsbC),
        );
        if next_window_mode != self.window_mode {
            self.window_mode = next_window_mode;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                self.window_mode.inner_size(),
            ));
            ctx.request_repaint();
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets;
    use readout_core::chart_pipeline::ChartPipeline;
    use readout_core::measurement_mode::MeasurementMode;
    use readout_core::types::DeviceMeasurement;
    use std::time::Instant;

    fn sample_measurement(device: DeviceId) -> DeviceMeasurement {
        match device {
            DeviceId::Multimeter => DeviceMeasurement {
                timestamp: Instant::now(),
                device,
                primary_value: Some(12.345),
                primary_unit: "V".into(),
                secondary_value: None,
                secondary_unit: None,
                power_watts: None,
                energy_mwh: None,
                energy_mah: None,
                mode: MeasurementMode::DcVoltage,
                mode_string: "VOLT:DC".into(),
                is_overload: false,
                is_open: false,
                is_short: false,
                alarm_state: AlarmState::None,
            },
            DeviceId::UsbC => DeviceMeasurement {
                timestamp: Instant::now(),
                device,
                primary_value: Some(5.123),
                primary_unit: "V".into(),
                secondary_value: Some(1.456),
                secondary_unit: Some("A".into()),
                power_watts: Some(7.462),
                energy_mwh: Some(123.4),
                energy_mah: Some(23.4),
                mode: MeasurementMode::DcVoltage,
                mode_string: "PD".into(),
                is_overload: false,
                is_open: false,
                is_short: false,
                alarm_state: AlarmState::None,
            },
        }
    }

    fn measured_content_height(
        show_mm: bool,
        show_usbc: bool,
        mm_alarm: AlarmState,
        usbc_alarm: AlarmState,
    ) -> f32 {
        let mut height = 0.0;
        let ctx = egui::Context::default();
        crate::theme::apply_theme(&ctx, readout_persistence::config::DashboardTheme::default());

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let content = ui.scope(|ui| {
                    let mut toolbar_state = widgets::toolbar::ToolbarState {
                        show_mm,
                        show_usbc,
                        paused: false,
                        pc_beep_enabled: false,
                        meter_beep_enabled: false,
                        selected_range_idx: 0,
                        show_log: false,
                        always_on_top: false,
                    };
                    widgets::toolbar::show(ui, &mut toolbar_state);
                    ui.separator();

                    if show_mm {
                        let mut mm_pipeline = ChartPipeline::new(64);
                        widgets::device_section::show(
                            ui,
                            DeviceId::Multimeter,
                            Some(&sample_measurement(DeviceId::Multimeter)),
                            &ConnectionState::Connected,
                            mm_alarm,
                            Some(&mut mm_pipeline),
                            0,
                            UsbCMetric::Voltage,
                        );
                    }

                    if show_mm && show_usbc {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }

                    if show_usbc {
                        let mut usbc_pipeline = ChartPipeline::new(64);
                        widgets::device_section::show(
                            ui,
                            DeviceId::UsbC,
                            Some(&sample_measurement(DeviceId::UsbC)),
                            &ConnectionState::Connected,
                            usbc_alarm,
                            Some(&mut usbc_pipeline),
                            0,
                            UsbCMetric::Voltage,
                        );
                    }
                });

                let panel_margin = ui.style().spacing.window_margin;
                height =
                    content.response.rect.height() + panel_margin.top as f32 + panel_margin.bottom as f32;
            });
        });

        height
    }

    #[test]
    fn measured_layout_heights_match_window_modes() {
        let mm = measured_content_height(true, false, AlarmState::None, AlarmState::None);
        let mm_alarm = measured_content_height(true, false, AlarmState::Open, AlarmState::None);
        let usbc = measured_content_height(false, true, AlarmState::None, AlarmState::None);
        let usbc_alarm = measured_content_height(false, true, AlarmState::None, AlarmState::HighAlarm);
        let both = measured_content_height(true, true, AlarmState::None, AlarmState::None);
        let both_mm_alarm = measured_content_height(true, true, AlarmState::Short, AlarmState::None);
        let both_usbc_alarm = measured_content_height(true, true, AlarmState::None, AlarmState::LowAlarm);

        assert_eq!(mm, WINDOW_HEIGHT_MM);
        assert_eq!(mm_alarm, WINDOW_HEIGHT_MM_ALARM);
        assert_eq!(usbc, WINDOW_HEIGHT_USBC - 20.0);
        assert_eq!(usbc_alarm, WINDOW_HEIGHT_USBC);
        assert_eq!(both, WINDOW_HEIGHT_BOTH - 20.0);
        assert_eq!(both_mm_alarm, WINDOW_HEIGHT_BOTH);
        assert_eq!(both_usbc_alarm, WINDOW_HEIGHT_BOTH);

        assert_eq!(
            CompactWindowMode::from_state(true, false, AlarmState::None, AlarmState::None).height(),
            WINDOW_HEIGHT_MM,
        );
        assert_eq!(
            CompactWindowMode::from_state(true, false, AlarmState::Short, AlarmState::None).height(),
            WINDOW_HEIGHT_MM_ALARM,
        );
        assert_eq!(
            CompactWindowMode::from_state(false, true, AlarmState::None, AlarmState::None).height(),
            WINDOW_HEIGHT_USBC,
        );
        assert_eq!(
            CompactWindowMode::from_state(false, true, AlarmState::None, AlarmState::HighAlarm).height(),
            WINDOW_HEIGHT_USBC,
        );
        assert_eq!(
            CompactWindowMode::from_state(true, true, AlarmState::None, AlarmState::None).height(),
            WINDOW_HEIGHT_BOTH,
        );
        assert_eq!(
            CompactWindowMode::from_state(true, true, AlarmState::Short, AlarmState::LowAlarm).height(),
            WINDOW_HEIGHT_BOTH,
        );
    }
}
