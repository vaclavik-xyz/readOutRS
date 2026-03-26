use readout_io::port_discovery::PortDiscovery;
use readout_persistence::config::AppConfiguration;
use readout_persistence::config_validator::{ConfigValidator, IssueSeverity};

pub struct FirstRunWizard {
    pub active: bool,
    draft: AppConfiguration,
    ports: Vec<String>,
    validation_messages: Vec<String>,
}

impl FirstRunWizard {
    pub fn new(config: &AppConfiguration, show: bool) -> Self {
        let ports: Vec<String> = PortDiscovery::scan()
            .into_iter()
            .map(|p| p.port_name)
            .collect();

        Self {
            active: show,
            draft: config.clone(),
            ports,
            validation_messages: Vec::new(),
        }
    }

    /// Returns Some(config) when user finishes wizard.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<AppConfiguration> {
        if !self.active {
            return None;
        }

        let mut result = None;

        egui::Window::new("Welcome to readout")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading("Setup");
                ui.label("Configure your measurement devices to get started.");
                ui.add_space(10.0);

                // Mode picker
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    ui.radio_value(&mut self.draft.use_simulator, true, "Simulator");
                    ui.radio_value(&mut self.draft.use_simulator, false, "Hardware");
                });

                ui.add_space(5.0);

                if self.draft.use_simulator {
                    ui.label("Simulator mode — no hardware needed.");
                } else {
                    // Device toggles
                    ui.checkbox(&mut self.draft.multimeter_enabled, "Multimeter");
                    if self.draft.multimeter_enabled {
                        port_selector(ui, "MM Port:", &self.ports, &mut self.draft.multimeter_port);
                    }

                    ui.checkbox(&mut self.draft.usbc_enabled, "USB-C Power Meter");
                    if self.draft.usbc_enabled {
                        port_selector(
                            ui,
                            "USB-C Port:",
                            &self.ports,
                            &mut self.draft.usbc_port,
                        );
                    }

                    if ui.button("Rescan ports").clicked() {
                        self.ports = PortDiscovery::scan()
                            .into_iter()
                            .map(|p| p.port_name)
                            .collect();
                    }
                }

                // Validation feedback
                if !self.validation_messages.is_empty() {
                    ui.add_space(5.0);
                    for msg in &self.validation_messages {
                        ui.colored_label(egui::Color32::RED, msg);
                    }
                }

                ui.add_space(10.0);

                if ui.button("Start").clicked() {
                    self.draft.clamp_values();
                    let issues = ConfigValidator::validate(&self.draft);
                    let errors: Vec<String> = issues
                        .iter()
                        .filter(|i| i.severity == IssueSeverity::Error)
                        .map(|i| i.message.clone())
                        .collect();

                    if errors.is_empty() {
                        result = Some(self.draft.clone());
                        self.active = false;
                    } else {
                        self.validation_messages = errors;
                    }
                }
            });

        result
    }
}

fn port_selector(ui: &mut egui::Ui, label: &str, ports: &[String], selected: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(label)
            .selected_text(if selected.is_empty() {
                "Select port..."
            } else {
                selected.as_str()
            })
            .show_ui(ui, |ui| {
                for port in ports {
                    ui.selectable_value(selected, port.clone(), port);
                }
            });
        // Manual entry fallback
        ui.text_edit_singleline(selected);
    });
}
