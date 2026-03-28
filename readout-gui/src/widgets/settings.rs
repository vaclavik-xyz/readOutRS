use egui_phosphor::regular as icons;
use readout_persistence::config::{AppConfiguration, DashboardDeviceVisibility, ObsOutputMode};

pub struct SettingsPanel {
    pub open: bool,
    prev_config: Option<AppConfiguration>,
}

fn file_picker_row(ui: &mut egui::Ui, path: &mut String) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(path).desired_width(ui.available_width() - 70.0));
        if ui.button(format!("{} Browse", icons::FOLDER_OPEN)).clicked() {
            let dialog = rfd::FileDialog::new().set_file_name(
                std::path::Path::new(path.as_str())
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("output.txt"),
            );
            if let Some(p) = dialog.save_file() {
                *path = p.display().to_string();
            }
        }
    });
}

impl SettingsPanel {
    pub fn new(_config: &AppConfiguration) -> Self {
        Self {
            open: false,
            prev_config: None,
        }
    }

    pub fn open_with(&mut self, _config: &AppConfiguration) {
        self.open = true;
    }

    /// Edits config directly. Returns true if config changed this frame.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        config: &mut AppConfiguration,
        theme: readout_persistence::config::DashboardTheme,
        parent_always_on_top: bool,
    ) -> bool {
        if !self.open {
            return false;
        }

        // Snapshot config at start of frame for change detection
        if self.prev_config.is_none() {
            self.prev_config = Some(config.clone());
        }

        let mut close_requested = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("settings"),
            {
                let mut vp = egui::ViewportBuilder::default()
                    .with_title("Settings")
                    .with_inner_size([360.0, 400.0])
                    .with_resizable(true);
                if parent_always_on_top {
                    vp = vp.with_always_on_top();
                }
                vp
            },
            |ctx, _class| {
                close_requested = ctx.input(|i| i.viewport().close_requested());
                crate::theme::apply_theme(ctx, theme);
                egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // --- Devices ---
                    ui.collapsing("Devices", |ui| {
                        ui.checkbox(&mut config.use_simulator, "Simulator mode");
                        ui.separator();
                        ui.checkbox(&mut config.multimeter_enabled, "Multimeter enabled");
                        ui.label("MM port:");
                        ui.text_edit_singleline(&mut config.multimeter_port);
                        ui.checkbox(&mut config.multimeter_auto_reconnect, "Auto reconnect");
                        ui.separator();
                        ui.checkbox(&mut config.usbc_enabled, "USB-C enabled");
                        ui.label("USB-C port:");
                        ui.text_edit_singleline(&mut config.usbc_port);
                        ui.checkbox(&mut config.usbc_auto_reconnect, "Auto reconnect");
                    });

                    // --- Sampling ---
                    ui.collapsing("Sampling", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Sample rate (Hz):");
                            ui.add(egui::DragValue::new(&mut config.sample_rate_hz).range(1..=50));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Graph history (s):");
                            ui.add(egui::DragValue::new(&mut config.graph_history_seconds).range(5..=600));
                        });
                    });

                    // --- Alarms ---
                    ui.collapsing("Alarms", |ui| {
                        ui.checkbox(&mut config.dcv_high_alarm_enabled, "DC voltage high alarm");
                        if config.dcv_high_alarm_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Threshold:");
                                ui.add(egui::DragValue::new(&mut config.dcv_high_alarm_value).speed(0.1));
                            });
                        }
                        ui.checkbox(&mut config.dcv_low_alarm_enabled, "DC voltage low alarm");
                        if config.dcv_low_alarm_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Threshold:");
                                ui.add(egui::DragValue::new(&mut config.dcv_low_alarm_value).speed(0.1));
                            });
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Short threshold (Ω):");
                            ui.add(egui::DragValue::new(&mut config.short_threshold).speed(0.1).range(0.1..=1000.0));
                        });
                        ui.checkbox(&mut config.beep_on_alarm, "Beep on alarm");
                        ui.checkbox(&mut config.beep_on_short_pc, "Beep on short (PC)");
                        ui.checkbox(&mut config.beep_on_short_meter, "Beep on short (meter)");
                    });

                    // --- CSV Logging ---
                    ui.collapsing("CSV Logging", |ui| {
                        ui.checkbox(&mut config.multimeter_csv_logging_enabled, "Multimeter CSV");
                        if config.multimeter_csv_logging_enabled {
                            file_picker_row(ui, &mut config.multimeter_csv_log_file_path);
                        }
                        ui.checkbox(&mut config.usbc_csv_logging_enabled, "USB-C CSV");
                        if config.usbc_csv_logging_enabled {
                            file_picker_row(ui, &mut config.usbc_csv_log_file_path);
                        }
                    });

                    // --- OBS Output ---
                    ui.collapsing("OBS Output", |ui| {
                        ui.label("MM file:");
                        file_picker_row(ui, &mut config.multimeter_output_file);
                        ui.add_space(4.0);
                        ui.label("USB-C file:");
                        file_picker_row(ui, &mut config.usbc_output_file);
                        ui.separator();

                        ui.label("MM output mode:");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut config.multimeter_obs_output_mode, ObsOutputMode::ValueOnly, "Value");
                            ui.selectable_value(&mut config.multimeter_obs_output_mode, ObsOutputMode::ValueAndUnit, "Value+Unit");
                            ui.selectable_value(&mut config.multimeter_obs_output_mode, ObsOutputMode::CustomTemplate, "Custom");
                        });
                        if config.multimeter_obs_output_mode == ObsOutputMode::CustomTemplate {
                            ui.text_edit_singleline(&mut config.multimeter_obs_custom_template);
                        }

                        ui.add_space(4.0);
                        ui.label("USB-C output mode:");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut config.usbc_obs_output_mode, ObsOutputMode::ValueOnly, "Value");
                            ui.selectable_value(&mut config.usbc_obs_output_mode, ObsOutputMode::ValueAndUnit, "Value+Unit");
                            ui.selectable_value(&mut config.usbc_obs_output_mode, ObsOutputMode::CustomTemplate, "Custom");
                        });
                        if config.usbc_obs_output_mode == ObsOutputMode::CustomTemplate {
                            ui.text_edit_singleline(&mut config.usbc_obs_custom_template);
                        }

                        ui.separator();
                        ui.label("MM label:");
                        ui.text_edit_singleline(&mut config.multimeter_value_label);
                        ui.add_space(4.0);
                        ui.label("USB-C label:");
                        ui.text_edit_singleline(&mut config.usbc_value_label);
                    });

                    // --- Appearance ---
                    ui.collapsing("Appearance", |ui| {
                        ui.label("Theme:");
                        ui.horizontal(|ui| {
                            use readout_persistence::config::DashboardTheme;
                            ui.selectable_value(&mut config.dashboard_theme, DashboardTheme::System, "System");
                            ui.selectable_value(&mut config.dashboard_theme, DashboardTheme::Dark, "Dark");
                            ui.selectable_value(&mut config.dashboard_theme, DashboardTheme::Light, "Light");
                        });
                        ui.label("Visibility:");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut config.dashboard_device_visibility, DashboardDeviceVisibility::Both, "Both");
                            ui.selectable_value(&mut config.dashboard_device_visibility, DashboardDeviceVisibility::Multimeter, "Multimeter");
                            ui.selectable_value(&mut config.dashboard_device_visibility, DashboardDeviceVisibility::UsbC, "USB-C");
                        });
                        ui.checkbox(&mut config.runtime_log_capture_enabled, "Capture runtime logs");
                    });

                    // --- Audio ---
                    ui.collapsing("Audio", |ui| {
                        ui.checkbox(&mut config.dashboard_beep_master_enabled, "Master beep enabled");
                        ui.horizontal(|ui| {
                            ui.label("Volume:");
                            ui.add(egui::Slider::new(&mut config.pc_beep_volume, 0.0..=1.0));
                        });
                    });
                });
                });
            },
        );

        if close_requested {
            self.open = false;
        }

        // Detect changes
        let changed = self.prev_config.as_ref() != Some(config);
        if changed {
            config.clamp_values();
            self.prev_config = Some(config.clone());
        }
        changed
    }
}
