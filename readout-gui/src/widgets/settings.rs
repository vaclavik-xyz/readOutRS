use readout_persistence::config::AppConfiguration;

pub struct SettingsPanel {
    pub open: bool,
    draft: AppConfiguration,
}

impl SettingsPanel {
    pub fn new(config: &AppConfiguration) -> Self {
        Self {
            open: false,
            draft: config.clone(),
        }
    }

    pub fn open_with(&mut self, config: &AppConfiguration) {
        self.draft = config.clone();
        self.open = true;
    }

    /// Returns Some(config) if user saved, None otherwise.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<AppConfiguration> {
        let mut result = None;

        if !self.open {
            return None;
        }

        let mut open = self.open;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // --- Devices ---
                    ui.collapsing("Devices", |ui| {
                        ui.checkbox(&mut self.draft.use_simulator, "Simulator mode");
                        ui.separator();
                        ui.checkbox(&mut self.draft.multimeter_enabled, "Multimeter enabled");
                        ui.horizontal(|ui| {
                            ui.label("Port:");
                            ui.text_edit_singleline(&mut self.draft.multimeter_port);
                        });
                        ui.checkbox(
                            &mut self.draft.multimeter_auto_reconnect,
                            "Auto reconnect",
                        );
                        ui.separator();
                        ui.checkbox(&mut self.draft.usbc_enabled, "USB-C enabled");
                        ui.horizontal(|ui| {
                            ui.label("Port:");
                            ui.text_edit_singleline(&mut self.draft.usbc_port);
                        });
                        ui.checkbox(&mut self.draft.usbc_auto_reconnect, "Auto reconnect");
                    });

                    // --- Sampling ---
                    ui.collapsing("Sampling", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Sample rate (Hz):");
                            ui.add(egui::DragValue::new(&mut self.draft.sample_rate_hz).range(1..=50));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Graph history (s):");
                            ui.add(
                                egui::DragValue::new(&mut self.draft.graph_history_seconds)
                                    .range(5..=600),
                            );
                        });
                    });

                    // --- Alarms ---
                    ui.collapsing("Alarms", |ui| {
                        ui.checkbox(
                            &mut self.draft.dcv_high_alarm_enabled,
                            "DC voltage high alarm",
                        );
                        if self.draft.dcv_high_alarm_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Threshold:");
                                ui.add(egui::DragValue::new(&mut self.draft.dcv_high_alarm_value).speed(0.1));
                            });
                        }
                        ui.checkbox(
                            &mut self.draft.dcv_low_alarm_enabled,
                            "DC voltage low alarm",
                        );
                        if self.draft.dcv_low_alarm_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Threshold:");
                                ui.add(egui::DragValue::new(&mut self.draft.dcv_low_alarm_value).speed(0.1));
                            });
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Short threshold (Ω):");
                            ui.add(
                                egui::DragValue::new(&mut self.draft.short_threshold)
                                    .speed(0.1)
                                    .range(0.1..=1000.0),
                            );
                        });
                        ui.checkbox(&mut self.draft.beep_on_alarm, "Beep on alarm");
                        ui.checkbox(&mut self.draft.beep_on_short_pc, "Beep on short (PC)");
                        ui.checkbox(
                            &mut self.draft.beep_on_short_meter,
                            "Beep on short (meter)",
                        );
                    });

                    // --- CSV Logging ---
                    ui.collapsing("CSV Logging", |ui| {
                        ui.checkbox(
                            &mut self.draft.multimeter_csv_logging_enabled,
                            "Multimeter CSV",
                        );
                        if self.draft.multimeter_csv_logging_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Path:");
                                ui.text_edit_singleline(
                                    &mut self.draft.multimeter_csv_log_file_path,
                                );
                            });
                        }
                        ui.checkbox(
                            &mut self.draft.usbc_csv_logging_enabled,
                            "USB-C CSV",
                        );
                        if self.draft.usbc_csv_logging_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Path:");
                                ui.text_edit_singleline(&mut self.draft.usbc_csv_log_file_path);
                            });
                        }
                    });

                    // --- OBS Output ---
                    ui.collapsing("OBS Output", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Multimeter file:");
                            ui.text_edit_singleline(&mut self.draft.multimeter_output_file);
                        });
                        ui.horizontal(|ui| {
                            ui.label("USB-C file:");
                            ui.text_edit_singleline(&mut self.draft.usbc_output_file);
                        });
                    });

                    // --- Appearance ---
                    ui.collapsing("Appearance", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Theme:");
                            use readout_persistence::config::DashboardTheme;
                            ui.selectable_value(&mut self.draft.dashboard_theme, DashboardTheme::System, "System");
                            ui.selectable_value(&mut self.draft.dashboard_theme, DashboardTheme::Dark, "Dark");
                            ui.selectable_value(&mut self.draft.dashboard_theme, DashboardTheme::Light, "Light");
                        });
                    });

                    // --- Audio ---
                    ui.collapsing("Audio", |ui| {
                        ui.checkbox(
                            &mut self.draft.dashboard_beep_master_enabled,
                            "Master beep enabled",
                        );
                        ui.horizontal(|ui| {
                            ui.label("Volume:");
                            ui.add(
                                egui::Slider::new(&mut self.draft.pc_beep_volume, 0.0..=1.0),
                            );
                        });
                    });

                    ui.separator();

                    // --- Save / Cancel ---
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.draft.clamp_values();
                            result = Some(self.draft.clone());
                            self.open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.open = false;
                        }
                    });
                });
            });

        self.open &= open;
        result
    }
}
