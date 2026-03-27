use crate::theme::colors;
use readout_core::dashboard_state::DashboardState;

pub fn show(ui: &mut egui::Ui, state: &DashboardState, is_simulator: bool) {
    ui.horizontal(|ui| {
        // Mode LED + label
        let (mode_color, mode_label) = if is_simulator {
            (egui::Color32::from_rgb(100, 160, 220), "SIM")
        } else {
            (colors::CONNECTED, "HW")
        };

        let (dot_rect, _) =
            ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        if ui.is_rect_visible(dot_rect) {
            ui.painter()
                .circle_filled(dot_rect.center(), 3.0, mode_color);
        }
        ui.label(
            egui::RichText::new(mode_label)
                .size(11.0)
                .family(egui::FontFamily::Monospace)
                .color(mode_color),
        );

        ui.separator();

        ui.label(format!("Measurements: {}", state.health.measurement_count));
        ui.separator();
        ui.label(format!("Errors: {}", state.health.error_count));
        ui.separator();
        ui.label(format!("Reconnects: {}", state.health.reconnect_count));

        if state.paused {
            ui.separator();
            ui.label(
                egui::RichText::new("⏸ PAUSED")
                    .strong()
                    .color(colors::CONNECTING),
            );
        }
    });
}
