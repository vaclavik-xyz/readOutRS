use crate::theme::{self, colors};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::LogLevel;

#[allow(dead_code)]
pub fn show(ctx: &egui::Context, state: &DashboardState, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Window::new("Log")
        .open(open)
        .resizable(true)
        .default_width(300.0)
        .default_height(250.0)
        .show(ctx, |ui| {
            // Health metrics header
            ui.label(
                egui::RichText::new(format!(
                    "Measurements: {} | Errors: {} | Reconnects: {}",
                    state.health.measurement_count,
                    state.health.error_count,
                    state.health.reconnect_count,
                ))
                .size(10.0)
                .color(theme::text_secondary(ui)),
            );
            ui.separator();

            egui::ScrollArea::vertical()
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
                                .size(11.0)
                                .color(color),
                        );
                    }
                });
        });
}
