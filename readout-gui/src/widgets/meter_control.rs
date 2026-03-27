use crate::theme;
use readout_core::dashboard_state::DashboardState;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{Command, MultimeterCommand, MultimeterRange, MultimeterRate};

pub struct MeterControlPanel {
    pub open: bool,
}

impl MeterControlPanel {
    pub fn new() -> Self {
        Self { open: false }
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &DashboardState,
    command_tx: Option<&tokio::sync::mpsc::Sender<Command>>,
    connected: bool,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.set_enabled(connected && command_tx.is_some());

        // Identity
        if let Some(ref identity) = state.meter_identity {
            let parts: Vec<&str> = identity.split(',').collect();
            let label = if parts.len() >= 4 {
                format!("{} {} · {}", parts[0].trim(), parts[1].trim(), parts[3].trim())
            } else {
                identity.clone()
            };
            ui.label(
                egui::RichText::new(label)
                    .size(11.0)
                    .color(theme::text_secondary(ui)),
            );
            ui.separator();
        }

        // Mode section
        ui.label(egui::RichText::new("Mode").size(11.0).strong());
        ui.add_space(2.0);

        let current_mode = state.meter_mode;

        let row1: &[(MeasurementMode, &str)] = &[
            (MeasurementMode::DcVoltage, "V DC"),
            (MeasurementMode::AcVoltage, "V AC"),
            (MeasurementMode::DcCurrent, "A DC"),
            (MeasurementMode::AcCurrent, "A AC"),
        ];
        let row2: &[(MeasurementMode, &str)] = &[
            (MeasurementMode::Resistance, "Ω"),
            (MeasurementMode::Capacitance, "Cap"),
            (MeasurementMode::Frequency, "Hz"),
            (MeasurementMode::Diode, "Diod"),
            (MeasurementMode::Continuity, "Cont"),
        ];

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (mode, label) in row1 {
                if ui.selectable_label(current_mode == *mode, egui::RichText::new(*label).size(11.0)).clicked() {
                    send_command(command_tx, MultimeterCommand::SetMode(*mode));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (mode, label) in row2 {
                if ui.selectable_label(current_mode == *mode, egui::RichText::new(*label).size(11.0)).clicked() {
                    send_command(command_tx, MultimeterCommand::SetMode(*mode));
                }
            }
        });

        ui.add_space(4.0);
        ui.separator();

        // Range section
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Range").size(11.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut auto = state.meter_auto_range;
                if ui.checkbox(&mut auto, "Auto").changed() {
                    let range = if auto { MultimeterRange::Auto } else { MultimeterRange::Manual(3) };
                    send_command(command_tx, MultimeterCommand::SetRange(range));
                }
            });
        });

        ui.set_enabled(connected && command_tx.is_some() && !state.meter_auto_range);
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("◀").size(14.0)).clicked() {
                send_command(command_tx, MultimeterCommand::SetRange(MultimeterRange::Manual(1)));
            }
            ui.label(
                egui::RichText::new(if state.meter_range_label.is_empty() { "---" } else { &state.meter_range_label })
                    .size(14.0)
                    .family(egui::FontFamily::Monospace),
            );
            if ui.button(egui::RichText::new("▶").size(14.0)).clicked() {
                send_command(command_tx, MultimeterCommand::SetRange(MultimeterRange::Manual(7)));
            }
        });
        ui.set_enabled(connected && command_tx.is_some());

        ui.add_space(4.0);
        ui.separator();

        // Rate section
        ui.label(egui::RichText::new("Rate").size(11.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let current_rate = state.meter_rate;
            for (rate, label) in &[(MultimeterRate::Fast, "Fast"), (MultimeterRate::Medium, "Medium"), (MultimeterRate::Slow, "Slow")] {
                if ui.selectable_label(current_rate == *rate, egui::RichText::new(*label).size(11.0)).clicked() {
                    send_command(command_tx, MultimeterCommand::SetRate(*rate));
                }
            }
        });
    });
}

fn send_command(command_tx: Option<&tokio::sync::mpsc::Sender<Command>>, cmd: MultimeterCommand) {
    if let Some(tx) = command_tx {
        let _ = tx.try_send(Command::Meter(cmd));
    }
}
