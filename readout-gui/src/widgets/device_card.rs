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

    let frame_fill = match alarm {
        AlarmState::HighAlarm | AlarmState::LowAlarm => {
            egui::Color32::from_rgba_premultiplied(180, 60, 60, 40)
        }
        AlarmState::Short => egui::Color32::from_rgba_premultiplied(200, 120, 0, 40),
        AlarmState::Open => egui::Color32::from_rgba_premultiplied(160, 160, 0, 40),
        AlarmState::None => egui::Color32::TRANSPARENT,
    };

    egui::Frame::group(ui.style())
        .fill(frame_fill)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // Header row: title + connection pill
                ui.horizontal(|ui| {
                    ui.strong(title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        connection_pill(ui, connection);
                    });
                });

                ui.add_space(4.0);

                if let Some(m) = measurement {
                    // Primary value with SI prefix
                    let value_text = m
                        .primary_value
                        .map(|v| format_si(v, &m.primary_unit))
                        .unwrap_or_else(|| format!("OL {}", m.primary_unit));

                    ui.label(
                        egui::RichText::new(&value_text)
                            .size(28.0)
                            .strong(),
                    );

                    // Mode
                    ui.label(
                        egui::RichText::new(&m.mode_string)
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );

                    // Secondary values (USB-C)
                    if device == DeviceId::UsbC {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        if let (Some(current), Some(power)) = (m.secondary_value, m.power_watts) {
                            ui.horizontal(|ui| {
                                ui.label(format_si(current, "A"));
                                ui.separator();
                                ui.label(format_si(power, "W"));
                            });
                        }
                        if let Some(mwh) = m.energy_mwh {
                            ui.label(
                                egui::RichText::new(format!("{mwh:.1} mWh"))
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    }

                    // Alarm indicator
                    match alarm {
                        AlarmState::HighAlarm => {
                            ui.colored_label(egui::Color32::RED, "⚠ HIGH ALARM");
                        }
                        AlarmState::LowAlarm => {
                            ui.colored_label(egui::Color32::RED, "⚠ LOW ALARM");
                        }
                        AlarmState::Short => {
                            ui.colored_label(egui::Color32::from_rgb(200, 120, 0), "⚡ SHORT");
                        }
                        AlarmState::Open => {
                            ui.colored_label(egui::Color32::YELLOW, "⊘ OPEN");
                        }
                        AlarmState::None => {}
                    }
                } else {
                    // No data yet
                    ui.label(
                        egui::RichText::new("---")
                            .size(28.0)
                            .color(egui::Color32::GRAY),
                    );
                    disconnected_hint(ui, connection);
                }
            });
        });
}

fn connection_pill(ui: &mut egui::Ui, state: &ConnectionState) {
    let (color, text) = match state {
        ConnectionState::Connected => (egui::Color32::from_rgb(60, 180, 80), "Connected"),
        ConnectionState::Connecting => (egui::Color32::from_rgb(220, 180, 40), "Connecting..."),
        ConnectionState::Reconnecting => (egui::Color32::from_rgb(220, 180, 40), "Reconnecting..."),
        ConnectionState::Disconnected => (egui::Color32::from_rgb(120, 120, 120), "Disconnected"),
        ConnectionState::Error(_) => (egui::Color32::from_rgb(220, 60, 60), "Error"),
    };

    // Paint pill background then text on top
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );
    let desired = galley.size() + egui::vec2(12.0, 6.0);
    let (rect, _response) = ui.allocate_exact_size(desired, egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, color.linear_multiply(0.7));
    ui.painter().galley(
        rect.min + egui::vec2(6.0, 3.0),
        galley,
        egui::Color32::WHITE,
    );

    if let ConnectionState::Error(msg) = state {
        _response.on_hover_text(msg);
    }
}

fn disconnected_hint(ui: &mut egui::Ui, connection: &ConnectionState) {
    let hint = match connection {
        ConnectionState::Connecting | ConnectionState::Reconnecting => "Connecting...",
        ConnectionState::Error(_) => "Connection error — check device",
        ConnectionState::Disconnected => "Disconnected",
        ConnectionState::Connected => return,
    };
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(hint)
            .size(12.0)
            .color(egui::Color32::GRAY)
            .italics(),
    );
}
