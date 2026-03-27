use crate::theme::{self, colors};
use readout_core::types::{AlarmState, ConnectionState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;

pub fn show(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    alarm: AlarmState,
    connection: &ConnectionState,
) {
    let title = match device {
        DeviceId::Multimeter => "Multimeter",
        DeviceId::UsbC => "USB-C Power Meter",
    };

    // Card fill — solid base with alarm tint blended in
    let base = ui.visuals().widgets.noninteractive.bg_fill;
    let fill = match alarm {
        AlarmState::HighAlarm | AlarmState::LowAlarm => theme::tint(base, 200, 50, 50, 0.12),
        AlarmState::Short => theme::tint(base, 210, 120, 10, 0.12),
        AlarmState::Open => theme::tint(base, 180, 170, 20, 0.12),
        AlarmState::None => base,
    };

    // Border: device accent when connected, default otherwise
    let border_color = if matches!(connection, ConnectionState::Connected) {
        let c = match device {
            DeviceId::Multimeter => colors::MM_LINE,
            DeviceId::UsbC => colors::USBC_LINE,
        };
        theme::with_alpha(c, 70)
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };

    let shadow_alpha = if ui.visuals().dark_mode { 50 } else { 12 };

    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, border_color))
        .inner_margin(14.0)
        .corner_radius(6.0)
        .shadow(egui::Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: egui::Color32::from_black_alpha(shadow_alpha),
        })
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // Title + connection LED
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .size(13.0)
                            .strong()
                            .color(theme::text_secondary(ui)),
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            connection_indicator(ui, connection);
                        },
                    );
                });

                ui.add_space(8.0);

                if let Some(m) = measurement {
                    // Primary value — large monospace for instrument feel
                    let value_text = m
                        .primary_value
                        .map(|v| format_si(v, &m.primary_unit))
                        .unwrap_or_else(|| format!("OL {}", m.primary_unit));

                    ui.label(
                        egui::RichText::new(&value_text)
                            .size(38.0)
                            .strong()
                            .family(egui::FontFamily::Monospace),
                    );

                    ui.add_space(2.0);

                    // Mode
                    ui.label(
                        egui::RichText::new(&m.mode_string)
                            .size(11.0)
                            .color(theme::text_secondary(ui)),
                    );

                    // USB-C secondary values
                    if device == DeviceId::UsbC {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);
                        if let (Some(current), Some(power)) = (m.secondary_value, m.power_watts) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format_si(current, "A"))
                                        .size(17.0)
                                        .family(egui::FontFamily::Monospace),
                                );
                                ui.label(
                                    egui::RichText::new("|")
                                        .size(17.0)
                                        .color(theme::text_secondary(ui)),
                                );
                                ui.label(
                                    egui::RichText::new(format_si(power, "W"))
                                        .size(17.0)
                                        .family(egui::FontFamily::Monospace),
                                );
                            });
                        }
                        if let Some(mwh) = m.energy_mwh {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(format!("{mwh:.1} mWh"))
                                    .size(12.0)
                                    .color(theme::text_secondary(ui)),
                            );
                        }
                    }

                    // Alarm badge
                    show_alarm_badge(ui, alarm);
                } else {
                    ui.label(
                        egui::RichText::new("---")
                            .size(38.0)
                            .family(egui::FontFamily::Monospace)
                            .color(theme::text_secondary(ui)),
                    );
                    disconnected_hint(ui, connection);
                }
            });
        });
}

/// LED-dot indicator with glow effect for connected state.
fn connection_indicator(ui: &mut egui::Ui, state: &ConnectionState) {
    let (color, label) = match state {
        ConnectionState::Connected => (colors::CONNECTED, "Connected"),
        ConnectionState::Connecting => (colors::CONNECTING, "Connecting..."),
        ConnectionState::Reconnecting => (colors::CONNECTING, "Reconnecting..."),
        ConnectionState::Disconnected => (colors::DISCONNECTED, "Disconnected"),
        ConnectionState::Error(_) => (colors::ERROR, "Error"),
    };

    let font = egui::FontId::proportional(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, color);
    let text_size = galley.size();
    let dot_r = 3.5;
    let gap = 6.0;
    let pad = 2.0;
    let total_w = pad + dot_r * 2.0 + gap + text_size.x;
    let total_h = text_size.y;

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(total_w, total_h), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let dot_center = egui::pos2(rect.left() + pad + dot_r, rect.center().y);

        // Glow halo for connected state
        if matches!(state, ConnectionState::Connected) {
            ui.painter()
                .circle_filled(dot_center, 6.0, theme::with_alpha(color, 25));
        }
        ui.painter().circle_filled(dot_center, dot_r, color);

        let text_pos = egui::pos2(
            rect.left() + pad + dot_r * 2.0 + gap,
            rect.center().y - text_size.y / 2.0,
        );
        ui.painter().galley(text_pos, galley, color);
    }

    if let ConnectionState::Error(msg) = state {
        response.on_hover_text(msg);
    }
}

fn show_alarm_badge(ui: &mut egui::Ui, alarm: AlarmState) {
    let (icon, text, color) = match alarm {
        AlarmState::HighAlarm => ("▲", "HIGH ALARM", colors::ALARM_RED),
        AlarmState::LowAlarm => ("▼", "LOW ALARM", colors::ALARM_RED),
        AlarmState::Short => ("⚡", "SHORT", colors::ALARM_ORANGE),
        AlarmState::Open => ("○", "OPEN", colors::ALARM_YELLOW),
        AlarmState::None => return,
    };

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(format!("{icon} {text}"))
            .size(12.0)
            .strong()
            .color(color),
    );
}

fn disconnected_hint(ui: &mut egui::Ui, connection: &ConnectionState) {
    let hint = match connection {
        ConnectionState::Connecting | ConnectionState::Reconnecting => "Connecting...",
        ConnectionState::Error(_) => "Connection error",
        ConnectionState::Disconnected => "Disconnected",
        ConnectionState::Connected => return,
    };
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(hint)
            .size(12.0)
            .color(theme::text_secondary(ui))
            .italics(),
    );
}
