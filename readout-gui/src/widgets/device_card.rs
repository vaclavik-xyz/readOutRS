use readout_core::types::{AlarmState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;

pub fn show(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    alarm: AlarmState,
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
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.strong(title);

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
                        if let Some(current) = m.secondary_value {
                            ui.label(format!("{current:.3} A"));
                        }
                        if let Some(power) = m.power_watts {
                            ui.label(format!("{power:.2} W"));
                        }
                        if let Some(mwh) = m.energy_mwh {
                            ui.label(format!("{mwh:.0} mWh"));
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
                    ui.label(
                        egui::RichText::new("---")
                            .size(28.0)
                            .color(egui::Color32::GRAY),
                    );
                }
            });
        });
}
