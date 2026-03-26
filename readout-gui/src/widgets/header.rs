use readout_core::dashboard_state::DashboardState;
use readout_core::types::{ConnectionState, DeviceId};

pub enum HeaderAction {
    None,
    Stop,
    Start,
}

pub fn show(
    ui: &mut egui::Ui,
    state: &DashboardState,
    running: bool,
    paused: &mut bool,
) -> HeaderAction {
    let mut action = HeaderAction::None;

    ui.horizontal(|ui| {
        if running {
            if ui.button("⏹ Stop").clicked() {
                action = HeaderAction::Stop;
            }
        } else {
            ui.add_enabled(false, egui::Button::new("▶ Start"))
                .on_disabled_hover_text("Restart not yet implemented");
        }

        if ui
            .button(if *paused { "▶ Resume" } else { "⏸ Pause" })
            .clicked()
        {
            *paused = !*paused;
        }

        ui.separator();

        connection_badge(ui, "MM", state.connection_for(DeviceId::Multimeter));
        connection_badge(ui, "USB-C", state.connection_for(DeviceId::UsbC));
    });

    action
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
