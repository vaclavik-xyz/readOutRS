use readout_core::types::{DeviceId, DeviceMeasurement};

pub struct PopoutState {
    pub multimeter_open: bool,
    pub usbc_open: bool,
}

impl Default for PopoutState {
    fn default() -> Self {
        Self {
            multimeter_open: false,
            usbc_open: false,
        }
    }
}

pub fn show_popouts(
    ctx: &egui::Context,
    state: &mut PopoutState,
    multimeter: Option<&DeviceMeasurement>,
    usbc: Option<&DeviceMeasurement>,
) {
    if state.multimeter_open {
        show_popout_window(ctx, "Multimeter", DeviceId::Multimeter, multimeter, &mut state.multimeter_open);
    }
    if state.usbc_open {
        show_popout_window(ctx, "USB-C", DeviceId::UsbC, usbc, &mut state.usbc_open);
    }
}

fn show_popout_window(
    ctx: &egui::Context,
    title: &str,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    open: &mut bool,
) {
    egui::Window::new(format!("{title} — Popout"))
        .open(open)
        .default_size([350.0, 200.0])
        .show(ctx, |ui| {
            if let Some(m) = measurement {
                let value_text = m
                    .primary_value
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_else(|| "OL".into());

                ui.label(
                    egui::RichText::new(format!("{value_text} {}", m.primary_unit))
                        .size(48.0)
                        .strong(),
                );

                ui.label(
                    egui::RichText::new(&m.mode_string)
                        .size(16.0)
                        .color(egui::Color32::GRAY),
                );

                if device == DeviceId::UsbC {
                    if let Some(current) = m.secondary_value {
                        ui.label(egui::RichText::new(format!("{current:.3} A")).size(24.0));
                    }
                    if let Some(power) = m.power_watts {
                        ui.label(egui::RichText::new(format!("{power:.2} W")).size(24.0));
                    }
                    if let Some(mwh) = m.energy_mwh {
                        ui.label(format!("{mwh:.0} mWh"));
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new("---")
                        .size(48.0)
                        .color(egui::Color32::GRAY),
                );
            }
        });
}
