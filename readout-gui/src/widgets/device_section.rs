use crate::theme::{self, colors};
use crate::widgets::toolbar::RANGE_OPTIONS;
use readout_core::chart_pipeline::ChartPipeline;
use readout_core::dashboard_state::{UsbCMetric, USBC_METRICS};
use readout_core::types::{AlarmState, ConnectionState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;
use std::time::Duration;

#[derive(Default)]
pub enum SectionAction {
    #[default]
    None,
    ResetEnergy,
    SetUsbcMetric(UsbCMetric),
}

pub fn show(
    ui: &mut egui::Ui,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    connection: &ConnectionState,
    alarm: AlarmState,
    pipeline: Option<&mut ChartPipeline>,
    selected_range_idx: usize,
    usbc_metric: UsbCMetric,
) -> SectionAction {
    let mut action = SectionAction::None;

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
                                action = SectionAction::ResetEnergy;
                            }
                        });
                    }

                    // USB-C metric selector
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (metric, label) in USBC_METRICS {
                            let selected = usbc_metric == *metric;
                            if ui
                                .selectable_label(
                                    selected,
                                    egui::RichText::new(*label).size(10.0),
                                )
                                .clicked()
                            {
                                action = SectionAction::SetUsbcMetric(*metric);
                            }
                        }
                    });
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
                DeviceId::Multimeter => "mm_chart",
                DeviceId::UsbC => "usbc_chart",
            };

            let (range, _) = RANGE_OPTIONS[selected_range_idx];
            let target_points = (ui.available_width() as usize).max(100);
            let chart_data: Vec<[f64; 2]> = pipeline
                .map(|p| {
                    let now = p.latest_timestamp().unwrap_or(Duration::ZERO);
                    p.query_with_now(range, target_points, now)
                        .iter()
                        .map(|(t, v)| [t.as_secs_f64(), *v])
                        .collect()
                })
                .unwrap_or_default();

            egui_plot::Plot::new(chart_id)
                .height(80.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .show_axes([false, false])
                .show(ui, |plot_ui| {
                    if !chart_data.is_empty() {
                        plot_ui.line(
                            egui_plot::Line::new(title, chart_data)
                                .stroke(egui::Stroke::new(1.5, line_color)),
                        );
                    }
                });
        });

    action
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
