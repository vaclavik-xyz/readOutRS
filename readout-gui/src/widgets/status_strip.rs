use readout_core::dashboard_state::DashboardState;

pub fn show(ui: &mut egui::Ui, state: &DashboardState, is_simulator: bool) {
    ui.horizontal(|ui| {
        // Mode badge
        if is_simulator {
            ui.colored_label(egui::Color32::from_rgb(100, 160, 220), "SIM");
        } else {
            ui.colored_label(egui::Color32::from_rgb(60, 180, 80), "HW");
        }
        ui.separator();

        ui.label(format!("Measurements: {}", state.health.measurement_count));
        ui.separator();
        ui.label(format!("Errors: {}", state.health.error_count));
        ui.separator();
        ui.label(format!("Reconnects: {}", state.health.reconnect_count));

        if state.paused {
            ui.separator();
            ui.colored_label(egui::Color32::YELLOW, "⏸ PAUSED");
        }
    });
}
