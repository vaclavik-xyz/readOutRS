use readout_core::dashboard_state::DashboardState;
use readout_core::types::{ConnectionState, DeviceId};

pub fn show(ui: &mut egui::Ui, state: &DashboardState, running: &mut bool, paused: &mut bool) {
    ui.horizontal(|ui| {
        if *running {
            if ui.button("⏹ Stop").clicked() {
                *running = false;
            }
        } else if ui.button("▶ Start").clicked() {
            *running = true;
        }

        if ui.button(if *paused { "▶ Resume" } else { "⏸ Pause" }).clicked() {
            *paused = !*paused;
        }

        ui.separator();

        // Connection indicators
        connection_badge(ui, "MM", state.connection_for(DeviceId::Multimeter));
        connection_badge(ui, "USB-C", state.connection_for(DeviceId::UsbC));
    });
}

fn connection_badge(ui: &mut egui::Ui, label: &str, state: &ConnectionState) {
    let (color, symbol) = match state {
        ConnectionState::Connected => (egui::Color32::GREEN, "●"),
        ConnectionState::Connecting | ConnectionState::Reconnecting => {
            (egui::Color32::YELLOW, "◐")
        }
        ConnectionState::Disconnected => (egui::Color32::GRAY, "○"),
        ConnectionState::Error(_) => (egui::Color32::RED, "✖"),
    };
    ui.colored_label(color, format!("{symbol} {label}"));
}
