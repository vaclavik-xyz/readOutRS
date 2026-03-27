use crate::theme::{self, colors};
use crate::widgets::chart::RANGE_OPTIONS;
use readout_core::dashboard_state::{UsbCMetric, USBC_METRICS};
use readout_core::types::{AlarmState, ConnectionState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;

pub struct PopoutState {
    pub open: bool,
    pub show_mm: bool,
    pub show_usbc: bool,
}

impl Default for PopoutState {
    fn default() -> Self {
        Self {
            open: false,
            show_mm: true,
            show_usbc: true,
        }
    }
}

#[derive(Default)]
pub enum PopoutAction {
    #[default]
    None,
    TogglePause,
    TogglePcBeep,
    ToggleMeterBeep,
    ResetEnergy,
    SetUsbcMetric(UsbCMetric),
    SetTimeRange(usize),
}

pub struct PopoutInput {
    pub mm_measurement: Option<DeviceMeasurement>,
    pub usbc_measurement: Option<DeviceMeasurement>,
    pub mm_connection: ConnectionState,
    pub usbc_connection: ConnectionState,
    pub mm_alarm: AlarmState,
    pub usbc_alarm: AlarmState,
    pub mm_chart_data: Vec<[f64; 2]>,
    pub usbc_chart_data: Vec<[f64; 2]>,
    pub paused: bool,
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub usbc_metric: UsbCMetric,
    pub selected_range_idx: usize,
}

fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("popout_combined")
}

pub fn show_combined_popout(
    ctx: &egui::Context,
    state: &mut PopoutState,
    input: PopoutInput,
) -> PopoutAction {
    if !state.open {
        return PopoutAction::None;
    }

    let mut action = PopoutAction::None;

    ctx.show_viewport_immediate(
        viewport_id(),
        egui::ViewportBuilder::default()
            .with_title("readout")
            .with_inner_size([320.0, 500.0])
            .with_min_inner_size([280.0, 300.0])
            .with_always_on_top(),
        |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                state.open = false;
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                render_toolbar(ui, state, &input, &mut action);
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if state.show_mm {
                        render_device_section(
                            ui,
                            DeviceId::Multimeter,
                            input.mm_measurement.as_ref(),
                            &input.mm_connection,
                            input.mm_alarm,
                            &input.mm_chart_data,
                            &mut action,
                        );
                    }

                    if state.show_mm && state.show_usbc {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }

                    if state.show_usbc {
                        render_device_section(
                            ui,
                            DeviceId::UsbC,
                            input.usbc_measurement.as_ref(),
                            &input.usbc_connection,
                            input.usbc_alarm,
                            &input.usbc_chart_data,
                            &mut action,
                        );
                    }
                });
            });
        },
    );

    action
}

fn render_toolbar(
    ui: &mut egui::Ui,
    state: &mut PopoutState,
    input: &PopoutInput,
    action: &mut PopoutAction,
) {
    // Row 1: device visibility + pause + beep
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        if ui
            .selectable_label(state.show_mm, egui::RichText::new("MM").size(10.0))
            .clicked()
        {
            // Only allow toggling off when the other device is visible
            if !state.show_mm || state.show_usbc {
                state.show_mm = !state.show_mm;
            }
        }
        if ui
            .selectable_label(state.show_usbc, egui::RichText::new("USB-C").size(10.0))
            .clicked()
        {
            if !state.show_usbc || state.show_mm {
                state.show_usbc = !state.show_usbc;
            }
        }

        ui.separator();

        let pause_label = if input.paused { "▶" } else { "⏸" };
        if ui
            .button(egui::RichText::new(pause_label).size(10.0))
            .clicked()
        {
            *action = PopoutAction::TogglePause;
        }

        ui.separator();

        let pc_icon = if input.pc_beep_enabled { "🔊" } else { "🔇" };
        if ui
            .selectable_label(
                input.pc_beep_enabled,
                egui::RichText::new(format!("{pc_icon} PC")).size(10.0),
            )
            .clicked()
        {
            *action = PopoutAction::TogglePcBeep;
        }
        let meter_icon = if input.meter_beep_enabled {
            "🔔"
        } else {
            "🔇"
        };
        if ui
            .selectable_label(
                input.meter_beep_enabled,
                egui::RichText::new(format!("{meter_icon} M")).size(10.0),
            )
            .clicked()
        {
            *action = PopoutAction::ToggleMeterBeep;
        }
    });

    // Row 2: USB-C metric + time range
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;

        for (metric, label) in USBC_METRICS {
            let selected = input.usbc_metric == *metric;
            if ui
                .selectable_label(selected, egui::RichText::new(*label).size(10.0))
                .clicked()
            {
                *action = PopoutAction::SetUsbcMetric(*metric);
            }
        }

        ui.separator();

        for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
            let selected = i == input.selected_range_idx;
            if ui
                .selectable_label(selected, egui::RichText::new(*label).size(10.0))
                .clicked()
            {
                *action = PopoutAction::SetTimeRange(i);
            }
        }
    });
}

fn render_device_section(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    connection: &ConnectionState,
    alarm: AlarmState,
    chart_data: &[[f64; 2]],
    action: &mut PopoutAction,
) {
    let title = match device {
        DeviceId::Multimeter => "Multimeter",
        DeviceId::UsbC => "USB-C",
    };

    let base = ui.visuals().widgets.noninteractive.bg_fill;
    let fill = match alarm {
        AlarmState::HighAlarm | AlarmState::LowAlarm => theme::tint(base, 200, 50, 50, 0.12),
        AlarmState::Short => theme::tint(base, 210, 120, 10, 0.12),
        AlarmState::Open => theme::tint(base, 180, 170, 20, 0.12),
        AlarmState::None => base,
    };

    egui::Frame::new()
        .fill(fill)
        .inner_margin(8.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            // Title + LED
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .size(11.0)
                        .color(theme::text_secondary(ui)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    connection_led(ui, connection);
                });
            });

            ui.add_space(4.0);

            if let Some(m) = measurement {
                let value_text = m
                    .primary_value
                    .map(|v| format_si(v, &m.primary_unit))
                    .unwrap_or_else(|| format!("OL {}", m.primary_unit));

                ui.label(
                    egui::RichText::new(&value_text)
                        .size(28.0)
                        .strong()
                        .family(egui::FontFamily::Monospace),
                );

                ui.label(
                    egui::RichText::new(&m.mode_string)
                        .size(10.0)
                        .color(theme::text_secondary(ui)),
                );

                // USB-C secondary values + energy reset
                if device == DeviceId::UsbC {
                    ui.add_space(4.0);
                    if let (Some(current), Some(power)) = (m.secondary_value, m.power_watts) {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format_si(current, "A"))
                                    .size(14.0)
                                    .family(egui::FontFamily::Monospace),
                            );
                            ui.label(
                                egui::RichText::new("|")
                                    .size(14.0)
                                    .color(theme::text_secondary(ui)),
                            );
                            ui.label(
                                egui::RichText::new(format_si(power, "W"))
                                    .size(14.0)
                                    .family(egui::FontFamily::Monospace),
                            );
                        });
                    }
                    if let Some(mwh) = m.energy_mwh {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{mwh:.1} mWh"))
                                    .size(11.0)
                                    .color(theme::text_secondary(ui)),
                            );
                            if ui
                                .small_button("↺")
                                .on_hover_text("Reset energy counter")
                                .clicked()
                            {
                                *action = PopoutAction::ResetEnergy;
                            }
                        });
                    }
                }

                show_alarm_badge(ui, alarm);
            } else {
                ui.label(
                    egui::RichText::new("---")
                        .size(28.0)
                        .family(egui::FontFamily::Monospace)
                        .color(theme::text_secondary(ui)),
                );
            }

            // Mini chart
            ui.add_space(4.0);
            let line_color = match device {
                DeviceId::Multimeter => colors::MM_LINE,
                DeviceId::UsbC => colors::USBC_LINE,
            };
            let chart_id = match device {
                DeviceId::Multimeter => "popout_mm_chart",
                DeviceId::UsbC => "popout_usbc_chart",
            };
            egui_plot::Plot::new(chart_id)
                .height(80.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .show_axes([false, false])
                .show(ui, |plot_ui| {
                    if !chart_data.is_empty() {
                        plot_ui.line(
                            egui_plot::Line::new(title, chart_data.to_vec())
                                .stroke(egui::Stroke::new(1.5, line_color)),
                        );
                    }
                });
        });
}

fn connection_led(ui: &mut egui::Ui, state: &ConnectionState) {
    let color = match state {
        ConnectionState::Connected => colors::CONNECTED,
        ConnectionState::Connecting | ConnectionState::Reconnecting => colors::CONNECTING,
        ConnectionState::Disconnected => colors::DISCONNECTED,
        ConnectionState::Error(_) => colors::ERROR,
    };

    let (rect, response) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        if matches!(state, ConnectionState::Connected) {
            ui.painter()
                .circle_filled(rect.center(), 5.0, theme::with_alpha(color, 25));
        }
        ui.painter().circle_filled(rect.center(), 3.0, color);
    }
    if let ConnectionState::Error(msg) = state {
        response.on_hover_text(msg);
    }
}

fn show_alarm_badge(ui: &mut egui::Ui, alarm: AlarmState) {
    let (icon, text, color) = match alarm {
        AlarmState::HighAlarm => ("▲", "HIGH", colors::ALARM_RED),
        AlarmState::LowAlarm => ("▼", "LOW", colors::ALARM_RED),
        AlarmState::Short => ("⚡", "SHORT", colors::ALARM_ORANGE),
        AlarmState::Open => ("○", "OPEN", colors::ALARM_YELLOW),
        AlarmState::None => return,
    };
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!("{icon} {text}"))
            .size(11.0)
            .strong()
            .color(color),
    );
}
