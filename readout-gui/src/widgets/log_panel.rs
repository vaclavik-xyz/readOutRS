use crate::theme::{self, colors};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::LogLevel;

pub fn show(ui: &mut egui::Ui, state: &DashboardState) {
    egui::ScrollArea::vertical()
        .max_height(150.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if state.log_entries.is_empty() {
                ui.label(
                    egui::RichText::new("No log entries")
                        .color(theme::text_secondary(ui))
                        .italics(),
                );
                return;
            }
            for entry in &state.log_entries {
                let color = match entry.level {
                    LogLevel::Error => colors::ERROR,
                    LogLevel::Warning => colors::CONNECTING,
                    LogLevel::Info => ui.visuals().widgets.noninteractive.fg_stroke.color,
                    LogLevel::Debug => theme::text_secondary(ui),
                };
                ui.label(
                    egui::RichText::new(&entry.message)
                        .family(egui::FontFamily::Monospace)
                        .size(12.0)
                        .color(color),
                );
            }
        });
}
