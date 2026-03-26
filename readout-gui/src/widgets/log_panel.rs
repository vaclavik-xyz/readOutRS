use readout_core::dashboard_state::DashboardState;
use readout_core::types::LogLevel;

pub fn show(ui: &mut egui::Ui, state: &DashboardState) {
    egui::ScrollArea::vertical()
        .max_height(150.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if state.log_entries.is_empty() {
                ui.colored_label(egui::Color32::GRAY, "No log entries");
                return;
            }
            for entry in &state.log_entries {
                let color = match entry.level {
                    LogLevel::Error => egui::Color32::RED,
                    LogLevel::Warning => egui::Color32::YELLOW,
                    LogLevel::Info => egui::Color32::LIGHT_GRAY,
                    LogLevel::Debug => egui::Color32::DARK_GRAY,
                };
                ui.colored_label(color, &entry.message);
            }
        });
}
