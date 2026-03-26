use readout_core::types::{DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;

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

fn mm_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("popout_multimeter")
}
fn usbc_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("popout_usbc")
}

pub fn show_popouts(
    ctx: &egui::Context,
    state: &mut PopoutState,
    multimeter: Option<&DeviceMeasurement>,
    usbc: Option<&DeviceMeasurement>,
) {
    if state.multimeter_open {
        let m = multimeter.cloned();
        ctx.show_viewport_immediate(
            mm_viewport_id(),
            egui::ViewportBuilder::default()
                .with_title("Multimeter — readout")
                .with_inner_size([350.0, 200.0])
                .with_min_inner_size([250.0, 150.0]),
            |ctx, _class| {
                // Check if user closed the window via OS close button
                if ctx.input(|i| i.viewport().close_requested()) {
                    state.multimeter_open = false;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_popout_content(ui, DeviceId::Multimeter, m.as_ref());
                });
            },
        );
    }

    if state.usbc_open {
        let m = usbc.cloned();
        ctx.show_viewport_immediate(
            usbc_viewport_id(),
            egui::ViewportBuilder::default()
                .with_title("USB-C — readout")
                .with_inner_size([350.0, 220.0])
                .with_min_inner_size([250.0, 150.0]),
            |ctx, _class| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    state.usbc_open = false;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_popout_content(ui, DeviceId::UsbC, m.as_ref());
                });
            },
        );
    }
}

fn render_popout_content(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
) {
    if let Some(m) = measurement {
        let value_text = m
            .primary_value
            .map(|v| format_si(v, &m.primary_unit))
            .unwrap_or_else(|| format!("OL {}", m.primary_unit));

        ui.label(
            egui::RichText::new(&value_text)
                .size(48.0)
                .strong(),
        );

        ui.label(
            egui::RichText::new(&m.mode_string)
                .size(16.0)
                .color(egui::Color32::GRAY),
        );

        if device == DeviceId::UsbC {
            ui.add_space(4.0);
            if let Some(current) = m.secondary_value {
                ui.label(egui::RichText::new(format_si(current, "A")).size(24.0));
            }
            if let Some(power) = m.power_watts {
                ui.label(egui::RichText::new(format_si(power, "W")).size(24.0));
            }
            if let Some(mwh) = m.energy_mwh {
                ui.label(
                    egui::RichText::new(format!("{mwh:.1} mWh"))
                        .size(14.0)
                        .color(egui::Color32::GRAY),
                );
            }
        }
    } else {
        ui.label(
            egui::RichText::new("---")
                .size(48.0)
                .color(egui::Color32::GRAY),
        );
    }
}
