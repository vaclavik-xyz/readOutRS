use crate::theme::{self, colors};
use crate::widgets::toolbar::RANGE_OPTIONS;
use readout_core::chart_pipeline::ChartPipeline;
use readout_core::dashboard_state::{USBC_METRICS, UsbCMetric};
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

#[derive(Default)]
pub enum DeviceRecordingAction {
    #[default]
    None,
    ToggleCsvLogging(DeviceId),
    ToggleObsOutput(DeviceId),
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    device_id: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    connection: &ConnectionState,
    alarm: AlarmState,
    pipeline: Option<&mut ChartPipeline>,
    selected_range_idx: usize,
    usbc_metric: UsbCMetric,
    chart_visible: &mut bool,
    csv_configured: bool,
    csv_active: bool,
    obs_configured: bool,
    obs_active: bool,
    energy_elapsed: Option<Duration>,
) -> (
    SectionAction,
    super::toolbar::ToolbarAction,
    DeviceRecordingAction,
) {
    let mut action = SectionAction::None;
    let mut toolbar_action = super::toolbar::ToolbarAction::None;
    let mut recording_action = DeviceRecordingAction::None;

    // Scale value fonts with window width (base design = 340px)
    let scale = (ui.available_width() / 320.0).clamp(1.0, 2.5);
    let value_size = 32.0 * scale;
    let current_size = 22.0 * scale;
    let chart_height = 80.0 * scale;

    let title = match device_id {
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
                if csv_configured {
                    let (color, hover_text, action) = if csv_active {
                        (
                            crate::theme::colors::ERROR,
                            "Stop CSV logging",
                            DeviceRecordingAction::ToggleCsvLogging(device_id),
                        )
                    } else {
                        (
                            theme::text_secondary(ui),
                            "Start CSV logging",
                            DeviceRecordingAction::ToggleCsvLogging(device_id),
                        )
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(egui_phosphor::regular::RECORD)
                                    .size(10.0)
                                    .color(color),
                            )
                            .frame(false)
                            .min_size(egui::vec2(14.0, 14.0)),
                        )
                        .on_hover_text(hover_text)
                        .clicked()
                    {
                        recording_action = action;
                    }
                }
                if obs_configured {
                    let (color, hover_text, action) = if obs_active {
                        (
                            crate::theme::colors::CONNECTED,
                            "Stop OBS output",
                            DeviceRecordingAction::ToggleObsOutput(device_id),
                        )
                    } else {
                        (
                            theme::text_secondary(ui),
                            "Start OBS output",
                            DeviceRecordingAction::ToggleObsOutput(device_id),
                        )
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(egui_phosphor::regular::BROADCAST)
                                    .size(10.0)
                                    .color(color),
                            )
                            .frame(false)
                            .min_size(egui::vec2(14.0, 14.0)),
                        )
                        .on_hover_text(hover_text)
                        .clicked()
                    {
                        recording_action = action;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    connection_led(ui, connection);
                    ui.add_space(4.0);
                    let chart_icon = if *chart_visible {
                        egui_phosphor::regular::CHART_LINE
                    } else {
                        egui_phosphor::regular::CHART_LINE_DOWN
                    };
                    if ui
                        .selectable_label(
                            *chart_visible,
                            egui::RichText::new(chart_icon).size(14.0),
                        )
                        .on_hover_text(if *chart_visible {
                            "Hide chart"
                        } else {
                            "Show chart"
                        })
                        .clicked()
                    {
                        *chart_visible = !*chart_visible;
                    }
                    if device_id == DeviceId::Multimeter {
                        ui.add_space(4.0);
                        let ta = super::toolbar::mm_inline_control(ui);
                        if !matches!(ta, super::toolbar::ToolbarAction::None) {
                            toolbar_action = ta;
                        }
                    }
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
                        .size(value_size)
                        .strong()
                        .family(egui::FontFamily::Monospace),
                );

                if device_id != DeviceId::UsbC {
                    show_mode_badge(ui, &m.mode_string, alarm);
                }

                if device_id == DeviceId::UsbC {
                    if let Some(current) = m.secondary_value {
                        ui.label(
                            egui::RichText::new(format_si(current, "A"))
                                .size(current_size)
                                .family(egui::FontFamily::Monospace)
                                .color(colors::USBC_LINE),
                        );
                    }
                    if let Some(power) = m.power_watts {
                        ui.label(
                            egui::RichText::new(format_si(power, "W"))
                                .size(11.0)
                                .family(egui::FontFamily::Monospace)
                                .color(theme::text_secondary(ui)),
                        );
                    }
                    if let Some(mwh) = m.energy_mwh {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{mwh:.1} mWh"))
                                    .size(11.0)
                                    .family(egui::FontFamily::Monospace)
                                    .color(theme::text_secondary(ui)),
                            );
                            if let Some(mah) = m.energy_mah {
                                ui.label(
                                    egui::RichText::new(format!("· {mah:.1} mAh"))
                                        .size(11.0)
                                        .family(egui::FontFamily::Monospace)
                                        .color(theme::text_secondary(ui)),
                                );
                            }
                            if let Some(elapsed) = energy_elapsed {
                                let secs = elapsed.as_secs();
                                let label = if secs >= 3600 {
                                    format!("· {}h {:02}m", secs / 3600, (secs % 3600) / 60)
                                } else if secs >= 60 {
                                    format!("· {}m {:02}s", secs / 60, secs % 60)
                                } else {
                                    format!("· {secs}s")
                                };
                                ui.label(
                                    egui::RichText::new(label)
                                        .size(11.0)
                                        .family(egui::FontFamily::Monospace)
                                        .color(theme::text_secondary(ui)),
                                );
                            }
                            let (rect, reset_btn) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                            let color = if reset_btn.hovered() {
                                colors::ACCENT
                            } else {
                                theme::text_secondary(ui)
                            };
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "↺",
                                egui::FontId::proportional(11.0),
                                color,
                            );
                            if reset_btn.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if reset_btn.on_hover_text("Reset energy counter").clicked() {
                                action = SectionAction::ResetEnergy;
                            }
                        });
                    }

                    // USB-C metric selector (only when chart visible)
                    if *chart_visible {
                        ui.add_space(4.0);
                        metric_selector(ui, usbc_metric, &mut action);
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new("---")
                        .size(value_size)
                        .family(egui::FontFamily::Monospace)
                        .color(theme::text_secondary(ui)),
                );
                let hint = match connection {
                    ConnectionState::Connecting | ConnectionState::Reconnecting => "Connecting...",
                    ConnectionState::Error(_) => "Connection error",
                    ConnectionState::Disconnected => "Disconnected",
                    ConnectionState::Connected => "Waiting for data...",
                };
                ui.label(
                    egui::RichText::new(hint)
                        .size(10.0)
                        .color(theme::text_secondary(ui))
                        .italics(),
                );
            }

            // Mini chart
            if !*chart_visible {
                // Skip chart rendering entirely
                return;
            }
            ui.add_space(4.0);
            let line_color = match device_id {
                DeviceId::Multimeter => colors::MM_LINE,
                DeviceId::UsbC => colors::USBC_LINE,
            };
            let chart_id = match device_id {
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

            // Compute min/max/avg for labels
            let (y_min, y_max, y_avg) = if chart_data.is_empty() {
                (0.0, 0.0, 0.0)
            } else {
                let mut min = f64::MAX;
                let mut max = f64::MIN;
                let mut sum = 0.0;
                for &[_, y] in &chart_data {
                    if y < min {
                        min = y;
                    }
                    if y > max {
                        max = y;
                    }
                    sum += y;
                }
                (min, max, sum / chart_data.len() as f64)
            };

            // Chart with min/max labels on the right
            ui.horizontal(|ui| {
                let chart_width = ui.available_width() - 55.0;
                ui.allocate_ui(egui::vec2(chart_width, chart_height), |ui| {
                    egui_plot::Plot::new(chart_id)
                        .height(chart_height)
                        .allow_drag(false)
                        .allow_zoom(false)
                        .allow_scroll(false)
                        .show_axes([false, false])
                        .show_grid(false)
                        .show_background(false)
                        .auto_bounds(egui::Vec2b::new(true, true))
                        .show(ui, |plot_ui| {
                            if !chart_data.is_empty() {
                                plot_ui.line(
                                    egui_plot::Line::new(title, chart_data)
                                        .stroke(egui::Stroke::new(1.5, line_color)),
                                );
                            }
                        });
                });

                // Right-side min/max labels
                if y_min != 0.0 || y_max != 0.0 {
                    let sec = theme::text_secondary(ui);
                    let fmt = |v: f64| -> String {
                        let a = v.abs();
                        if a >= 1000.0 {
                            format!("{v:.1}")
                        } else if a >= 100.0 {
                            format!("{v:.2}")
                        } else {
                            format!("{v:.3}")
                        }
                    };
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        ui.label(
                            egui::RichText::new(fmt(y_max))
                                .size(9.0)
                                .family(egui::FontFamily::Monospace)
                                .color(sec),
                        );
                        let remaining = (ui.available_height() - 24.0).max(0.0);
                        ui.add_space(remaining / 2.0);
                        ui.label(
                            egui::RichText::new(fmt(y_avg))
                                .size(9.0)
                                .family(egui::FontFamily::Monospace)
                                .color(sec),
                        );
                        ui.add_space((ui.available_height() - 12.0).max(0.0));
                        ui.label(
                            egui::RichText::new(fmt(y_min))
                                .size(9.0)
                                .family(egui::FontFamily::Monospace)
                                .color(sec),
                        );
                    });
                }
            });
        });

    (action, toolbar_action, recording_action)
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

fn show_mode_badge(ui: &mut egui::Ui, mode: &str, alarm: AlarmState) {
    let (label, bg, fg) = match alarm {
        AlarmState::HighAlarm => (
            format!("{mode} · ▲ HIGH"),
            colors::ALARM_RED,
            egui::Color32::WHITE,
        ),
        AlarmState::LowAlarm => (
            format!("{mode} · ▼ LOW"),
            colors::ALARM_RED,
            egui::Color32::WHITE,
        ),
        AlarmState::Short => (
            format!("{mode} · ⚡ SHORT"),
            colors::ALARM_ORANGE,
            egui::Color32::WHITE,
        ),
        AlarmState::Open => (
            format!("{mode} · ○ OPEN"),
            colors::ALARM_YELLOW,
            egui::Color32::from_rgb(40, 40, 40),
        ),
        AlarmState::None => (
            mode.to_string(),
            ui.visuals().widgets.inactive.bg_fill,
            ui.visuals().widgets.inactive.fg_stroke.color,
        ),
    };
    egui::Frame::new()
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(9.0)
                    .family(egui::FontFamily::Monospace)
                    .strong()
                    .color(fg),
            );
        });
}

fn metric_selector(ui: &mut egui::Ui, current: UsbCMetric, action: &mut SectionAction) {
    let bg = {
        let p = ui.visuals().panel_fill;
        let lum = (p.r() as u16 + p.g() as u16 + p.b() as u16) / 3;
        let d: i16 = if lum > 128 { -20 } else { 20 };
        egui::Color32::from_rgb(
            (p.r() as i16 + d).clamp(0, 255) as u8,
            (p.g() as i16 + d).clamp(0, 255) as u8,
            (p.b() as i16 + d).clamp(0, 255) as u8,
        )
    };
    egui::Frame::new()
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(1, 0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                let r = egui::CornerRadius::same(4);
                ui.visuals_mut().widgets.hovered.corner_radius = r;
                ui.visuals_mut().widgets.active.corner_radius = r;
                ui.visuals_mut().widgets.inactive.corner_radius = r;

                for (i, (metric, label)) in USBC_METRICS.iter().enumerate() {
                    if i > 0 {
                        let h = ui.spacing().interact_size.y * 0.5;
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(1.0, h), egui::Sense::hover());
                        if ui.is_rect_visible(rect) {
                            let color = ui.visuals().widgets.noninteractive.bg_stroke.color;
                            ui.painter().line_segment(
                                [rect.center_top(), rect.center_bottom()],
                                egui::Stroke::new(1.0, color),
                            );
                        }
                    }
                    if ui
                        .selectable_label(
                            current == *metric,
                            egui::RichText::new(*label).size(10.0),
                        )
                        .clicked()
                    {
                        *action = SectionAction::SetUsbcMetric(*metric);
                    }
                }
            });
        });
}
