use readout_core::dashboard_state::DashboardState;

pub fn show(ui: &mut egui::Ui, state: &DashboardState) {
    ui.horizontal(|ui| {
        ui.label(format!("Measurements: {}", state.health.measurement_count));
        ui.separator();
        ui.label(format!("Errors: {}", state.health.error_count));
        ui.separator();
        ui.label(format!("Reconnects: {}", state.health.reconnect_count));

        if state.paused {
            ui.separator();
            ui.colored_label(egui::Color32::YELLOW, "PAUSED");
        }
    });
}
