use readout_core::dashboard_state::DashboardState;
use readout_core::types::{ConnectionState, DeviceId};

pub enum HeaderAction {
    None,
    Stop,
    OpenSettings,
    ToggleBeep,
    ToggleLog,
}

pub struct HeaderState {
    pub beep_enabled: bool,
    pub log_visible: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    state: &DashboardState,
    running: bool,
    paused: &mut bool,
    header_state: &HeaderState,
) -> HeaderAction {
    let mut action = HeaderAction::None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Left: title + status
        ui.strong(egui::RichText::new("readout").size(16.0));

        ui.separator();

        // Connection badges
        connection_badge(ui, "MM", state.connection_for(DeviceId::Multimeter));
        connection_badge(ui, "USB-C", state.connection_for(DeviceId::UsbC));

        ui.separator();

        // Transport controls
        if running {
            if ui.button("⏹ Stop").clicked() {
                action = HeaderAction::Stop;
            }
        }

        let pause_label = if *paused { "▶ Resume" } else { "⏸ Pause" };
        if ui.button(pause_label).clicked() {
            *paused = !*paused;
        }

        // Right-aligned controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("⚙ Settings").clicked() {
                action = HeaderAction::OpenSettings;
            }

            let log_label = if header_state.log_visible { "📋 Log ✓" } else { "📋 Log" };
            if ui.button(log_label).clicked() {
                action = HeaderAction::ToggleLog;
            }

            let beep_label = if header_state.beep_enabled { "🔔 Beep ✓" } else { "🔇 Beep" };
            if ui.button(beep_label).clicked() {
                action = HeaderAction::ToggleBeep;
            }
        });
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
