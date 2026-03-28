use crate::theme;
use egui_phosphor::regular as icons;
use readout_core::dashboard_state::DashboardState;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::{Command, DbReference, MathFunction, MultimeterCommand, MultimeterRange, MultimeterRate, TempSensorType, TempUnit, DB_REFERENCE_VALUES};

pub struct MeterControlPanel {
    pub open: bool,
    pub applied_theme: Option<readout_persistence::config::DashboardTheme>,
}

impl MeterControlPanel {
    pub fn new() -> Self {
        Self {
            open: false,
            applied_theme: None,
        }
    }
}

#[derive(Default)]
pub enum MeterControlAction {
    #[default]
    None,
    TogglePcBeep,
    ToggleMeterBeep,
}

pub fn show(
    ctx: &egui::Context,
    state: &DashboardState,
    command_tx: Option<&tokio::sync::mpsc::Sender<Command>>,
    connected: bool,
    pc_beep_enabled: bool,
    meter_beep_enabled: bool,
) -> MeterControlAction {
    let mut action = MeterControlAction::None;

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

        let row3: &[(MeasurementMode, &str)] = &[
            (MeasurementMode::Temperature, "Temp"),
            (MeasurementMode::Period, "Per"),
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (mode, label) in row3 {
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

        let range_enabled = connected && command_tx.is_some() && !state.meter_auto_range;
        ui.add_enabled_ui(range_enabled, |ui| {
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
        });

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

        ui.add_space(4.0);
        ui.separator();

        // Dual Display
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Dual Display").size(11.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut dual = state.meter_dual_display;
                if ui.checkbox(&mut dual, "Freq").changed() {
                    send_command(command_tx, MultimeterCommand::SetDualDisplay(dual));
                }
            });
        });

        // NULL/REL
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Relative").size(11.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut null = state.meter_null_enabled;
                if ui.checkbox(&mut null, "NULL").changed() {
                    send_command(command_tx, MultimeterCommand::SetNull(null));
                }
            });
        });

        // DC Voltage options
        if state.meter_mode == MeasurementMode::DcVoltage {
            ui.add_space(4.0);
            ui.separator();
            ui.label(egui::RichText::new("DC Voltage").size(11.0).strong());
            ui.horizontal(|ui| {
                let mut filt = state.meter_dc_filter;
                if ui.checkbox(&mut filt, "DC Filter").changed() {
                    send_command(command_tx, MultimeterCommand::SetDcFilter(filt));
                }
                let mut imp = state.meter_auto_impedance;
                if ui.checkbox(&mut imp, "Auto Z").changed() {
                    send_command(command_tx, MultimeterCommand::SetAutoImpedance(imp));
                }
            });
        }

        // Temperature config
        if state.meter_mode == MeasurementMode::Temperature {
            ui.add_space(4.0);
            ui.separator();
            ui.label(egui::RichText::new("Temperature").size(11.0).strong());
            ui.horizontal(|ui| {
                ui.label("Sensor:");
                if ui.selectable_label(false, "KITS90").clicked() {
                    send_command(command_tx, MultimeterCommand::SetTempSensorType(TempSensorType::Kits90));
                }
                if ui.selectable_label(false, "PT100").clicked() {
                    send_command(command_tx, MultimeterCommand::SetTempSensorType(TempSensorType::Pt100));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Unit:");
                if ui.selectable_label(false, "°C").clicked() {
                    send_command(command_tx, MultimeterCommand::SetTempUnit(TempUnit::Celsius));
                }
                if ui.selectable_label(false, "°F").clicked() {
                    send_command(command_tx, MultimeterCommand::SetTempUnit(TempUnit::Fahrenheit));
                }
                if ui.selectable_label(false, "K").clicked() {
                    send_command(command_tx, MultimeterCommand::SetTempUnit(TempUnit::Kelvin));
                }
            });
        }

        // Math/Statistics
        ui.add_space(4.0);
        ui.separator();
        ui.label(egui::RichText::new("Math").size(11.0).strong());
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let active = state.meter_math_function;
            if ui.selectable_label(active == Some(MathFunction::Average), "MIN/MAX").clicked() {
                if active == Some(MathFunction::Average) {
                    send_command(command_tx, MultimeterCommand::StopMath);
                } else {
                    send_command(command_tx, MultimeterCommand::StartMath(MathFunction::Average));
                }
            }
            if ui.selectable_label(active == Some(MathFunction::Null), "REL").clicked() {
                if active == Some(MathFunction::Null) {
                    send_command(command_tx, MultimeterCommand::StopMath);
                } else {
                    send_command(command_tx, MultimeterCommand::StartMath(MathFunction::Null));
                }
            }
            if ui.selectable_label(active == Some(MathFunction::Db), "dB").clicked() {
                if active == Some(MathFunction::Db) {
                    send_command(command_tx, MultimeterCommand::StopMath);
                } else {
                    send_command(command_tx, MultimeterCommand::StartMath(MathFunction::Db));
                }
            }
            if ui.selectable_label(active == Some(MathFunction::Dbm), "dBm").clicked() {
                if active == Some(MathFunction::Dbm) {
                    send_command(command_tx, MultimeterCommand::StopMath);
                } else {
                    send_command(command_tx, MultimeterCommand::StartMath(MathFunction::Dbm));
                }
            }
        });
        if matches!(state.meter_math_function, Some(MathFunction::Db) | Some(MathFunction::Dbm)) {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Ref:").size(10.0));
                egui::ComboBox::from_id_salt("db_ref")
                    .width(60.0)
                    .selected_text("600")
                    .show_ui(ui, |ui| {
                        for &ohms in DB_REFERENCE_VALUES {
                            if ui.selectable_label(false, format!("{ohms} \u{2126}")).clicked() {
                                send_command(command_tx, MultimeterCommand::SetDbReference(DbReference::Ohms(ohms)));
                            }
                        }
                    });
            });
        }
        if state.meter_math_function == Some(MathFunction::Average) {
            if let Some(stats) = &state.meter_math_stats {
                ui.horizontal(|ui| {
                    let sec = theme::text_secondary(ui);
                    ui.label(egui::RichText::new(format!("Min: {:.4}", stats.min)).size(10.0).color(sec));
                    ui.label(egui::RichText::new(format!("Max: {:.4}", stats.max)).size(10.0).color(sec));
                    ui.label(egui::RichText::new(format!("Avg: {:.4}", stats.avg)).size(10.0).color(sec));
                });
            }
            // Throttle stats query to ~1/sec (every 60th frame)
            if ctx.cumulative_frame_nr() % 60 == 0 {
                send_command(command_tx, MultimeterCommand::QueryMathStats);
            }
        }

        // Remote / Reset
        ui.add_space(4.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(format!("{} Lock Panel", icons::LOCK_KEY)).size(11.0)).clicked() {
                send_command(command_tx, MultimeterCommand::SetRemoteMode(true));
            }
            if ui.button(egui::RichText::new(format!("{} Unlock", icons::LOCK_KEY_OPEN)).size(11.0)).clicked() {
                send_command(command_tx, MultimeterCommand::SetRemoteMode(false));
            }
        });
        if ui.button(egui::RichText::new(format!("{} Reset Device", icons::ARROW_COUNTER_CLOCKWISE)).size(11.0)).clicked() {
            send_command(command_tx, MultimeterCommand::ResetDevice);
        }

        ui.add_space(4.0);
        ui.separator();

        // Beep section — always enabled (independent of meter connection)
        ui.add_enabled_ui(true, |ui| {
            ui.label(egui::RichText::new("Beep").size(11.0).strong());
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let pc_icon = if pc_beep_enabled { icons::SPEAKER_HIGH } else { icons::SPEAKER_SLASH };
                if ui
                    .selectable_label(pc_beep_enabled, egui::RichText::new(format!("{pc_icon} PC beep")).size(11.0))
                    .clicked()
                {
                    action = MeterControlAction::TogglePcBeep;
                }

                let meter_icon = if meter_beep_enabled { icons::BELL_RINGING } else { icons::BELL_SLASH };
                if ui
                    .selectable_label(meter_beep_enabled, egui::RichText::new(format!("{meter_icon} Meter beep")).size(11.0))
                    .clicked()
                {
                    action = MeterControlAction::ToggleMeterBeep;
                }
            });
        });
    });

    action
}

fn send_command(command_tx: Option<&tokio::sync::mpsc::Sender<Command>>, cmd: MultimeterCommand) {
    if let Some(tx) = command_tx {
        let _ = tx.try_send(Command::Meter(cmd));
    }
}
