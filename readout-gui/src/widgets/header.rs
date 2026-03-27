use crate::theme::colors;
use readout_core::dashboard_state::DashboardState;
use readout_core::types::{ConnectionState, DeviceId};

pub enum HeaderAction {
    None,
    Stop,
    OpenSettings,
    TogglePcBeep,
    ToggleMeterBeep,
    ToggleLog,
    TogglePopout,
}

pub struct HeaderState {
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub log_visible: bool,
    pub popout_open: bool,
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

        // Brand title in accent color
        ui.label(
            egui::RichText::new("readout")
                .size(16.0)
                .strong()
                .color(colors::ACCENT),
        );

        ui.separator();

        // LED-dot connection badges
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

        ui.separator();

        // Popout toggle — selectable for visual state
        if ui
            .selectable_label(header_state.popout_open, "⬒ Popout")
            .clicked()
        {
            action = HeaderAction::TogglePopout;
        }

        // Right-aligned controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("⚙ Settings").clicked() {
                action = HeaderAction::OpenSettings;
            }

            if ui
                .selectable_label(header_state.log_visible, "📋 Log")
                .clicked()
            {
                action = HeaderAction::ToggleLog;
            }

            ui.separator();

            let meter_icon = if header_state.meter_beep_enabled {
                "🔔"
            } else {
                "🔇"
            };
            if ui
                .selectable_label(
                    header_state.meter_beep_enabled,
                    format!("{meter_icon} Meter"),
                )
                .clicked()
            {
                action = HeaderAction::ToggleMeterBeep;
            }

            let pc_icon = if header_state.pc_beep_enabled {
                "🔊"
            } else {
                "🔇"
            };
            if ui
                .selectable_label(header_state.pc_beep_enabled, format!("{pc_icon} PC"))
                .clicked()
            {
                action = HeaderAction::TogglePcBeep;
            }
        });
    });

    action
}

fn connection_badge(ui: &mut egui::Ui, label: &str, state: &ConnectionState) {
    let color = match state {
        ConnectionState::Connected => colors::CONNECTED,
        ConnectionState::Connecting | ConnectionState::Reconnecting => colors::CONNECTING,
        ConnectionState::Disconnected => colors::DISCONNECTED,
        ConnectionState::Error(_) => colors::ERROR,
    };

    let font = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, color);
    let text_size = galley.size();
    let dot_r = 3.0;
    let gap = 5.0;
    let pad = 2.0;
    let total_w = pad + dot_r * 2.0 + gap + text_size.x;
    let total_h = text_size.y;

    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, total_h), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let dot_center = egui::pos2(rect.left() + pad + dot_r, rect.center().y);
        ui.painter().circle_filled(dot_center, dot_r, color);

        let text_pos = egui::pos2(
            rect.left() + pad + dot_r * 2.0 + gap,
            rect.center().y - text_size.y / 2.0,
        );
        ui.painter().galley(text_pos, galley, color);
    }
}
