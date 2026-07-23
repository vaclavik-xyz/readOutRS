use super::info_bar::{self, CursorInfo, MeasurementDelta, SelectionStats};
use egui::{Align2, Color32, Key, Stroke};
use egui_plot::{HLine, Line, LineStyle, PlotPoint, PlotUi, Polygon, Text, VLine};
use readout_core::csv_record::CsvRecord;

#[derive(Debug, Clone, Default)]
pub struct OverlayState {
    pub cursor_pos: Option<PlotPoint>,
    pub measurements: Vec<Measurement>,
    pub measuring_from: Option<PlotPoint>,
    pub selection: Option<(f64, f64)>,
    pub selecting_from: Option<f64>,
    pub markers: Vec<UserMarker>,
    pub editing_marker: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct Measurement {
    pub from: PlotPoint,
    pub to: PlotPoint,
}

#[derive(Debug, Clone)]
pub struct UserMarker {
    pub x: f64,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct ModeChangeMarker {
    pub x: f64,
    pub label: String,
    pub color: Color32,
}

pub fn active_measurement_delta(
    state: &OverlayState,
    cursor_pos: Option<PlotPoint>,
) -> Option<MeasurementDelta> {
    let measurement = match (state.measuring_from, cursor_pos) {
        (Some(from), Some(to)) => Some(Measurement { from, to }),
        _ => state.measurements.last().copied(),
    }?;

    Some(MeasurementDelta {
        dt: format_duration((measurement.to.x - measurement.from.x).abs()),
        dv: measurement.to.y - measurement.from.y,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn compute_selection_stats(
    records: &[CsvRecord],
    x_min: f64,
    x_max: f64,
) -> Option<SelectionStats> {
    let (x_min, x_max) = ordered_range(x_min, x_max);
    let values: Vec<f64> = records
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            let x = records[*idx].parsed_time.unwrap_or(*idx as f64);
            x >= x_min && x <= x_max
        })
        .filter_map(|(_, record)| record.value)
        .collect();

    compute_value_stats(&values)
}

pub fn compute_value_stats(values: &[f64]) -> Option<SelectionStats> {
    if values.is_empty() {
        return None;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - avg).powi(2))
        .sum::<f64>()
        / values.len() as f64;

    Some(SelectionStats {
        min,
        max,
        avg,
        stddev: variance.sqrt(),
    })
}

pub fn handle_measure_interaction(
    state: &mut OverlayState,
    response: &egui::Response,
    cursor_pos: Option<PlotPoint>,
) {
    if response.ctx.input(|input| input.key_pressed(Key::Escape))
        && state.measuring_from.take().is_none()
    {
        state.measurements.pop();
    }

    let Some(pos) = cursor_pos else {
        return;
    };

    if response.clicked() {
        if let Some(from) = state.measuring_from.take() {
            state.measurements.push(Measurement { from, to: pos });
        } else {
            state.measuring_from = Some(pos);
        }
    }
}

pub fn draw_measurements(
    plot_ui: &mut PlotUi<'_>,
    state: &OverlayState,
    measuring_cursor: Option<PlotPoint>,
) {
    for (idx, measurement) in state.measurements.iter().enumerate() {
        plot_ui.line(
            Line::new(
                format!("measurement_{idx}"),
                vec![
                    [measurement.from.x, measurement.from.y],
                    [measurement.to.x, measurement.to.y],
                ],
            )
            .color(Color32::YELLOW)
            .width(1.5_f32),
        );
    }

    if let (Some(from), Some(to)) = (state.measuring_from, measuring_cursor) {
        plot_ui.line(
            Line::new(
                "measurement_in_progress",
                vec![[from.x, from.y], [to.x, to.y]],
            )
            .color(Color32::YELLOW.linear_multiply(0.7))
            .width(1.0_f32)
            .style(LineStyle::dashed_dense()),
        );
    }
}

pub fn handle_select_interaction(
    state: &mut OverlayState,
    response: &egui::Response,
    cursor_pos: Option<PlotPoint>,
) {
    if response.ctx.input(|input| input.key_pressed(Key::Escape)) {
        state.selection = None;
        state.selecting_from = None;
    }

    let Some(pos) = cursor_pos else {
        if response.drag_stopped() {
            state.selecting_from = None;
        }
        return;
    };

    if response.drag_started() {
        state.selecting_from = Some(pos.x);
        state.selection = Some((pos.x, pos.x));
    }
    if response.dragged()
        && let Some(from_x) = state.selecting_from
    {
        state.selection = Some(ordered_range(from_x, pos.x));
    }
    if response.drag_stopped() {
        state.selecting_from = None;
    }
}

pub fn draw_selection(plot_ui: &mut PlotUi<'_>, selection: Option<(f64, f64)>) {
    let Some((x_min, x_max)) = selection.map(|(start, end)| ordered_range(start, end)) else {
        return;
    };

    let bounds = plot_ui.plot_bounds();
    if !bounds.is_valid() {
        return;
    }

    let y_min = bounds.min()[1];
    let y_max = bounds.max()[1];
    let fill = Color32::from_rgba_premultiplied(100, 150, 255, 30);
    let edge = Color32::from_rgba_premultiplied(120, 170, 255, 140);

    if (x_max - x_min).abs() > f64::EPSILON {
        plot_ui.polygon(
            Polygon::new(
                "selection_fill",
                vec![
                    [x_min, y_min],
                    [x_min, y_max],
                    [x_max, y_max],
                    [x_max, y_min],
                ],
            )
            .fill_color(fill)
            .stroke(Stroke::new(0.0_f32, Color32::TRANSPARENT)),
        );
    }

    plot_ui.vline(
        VLine::new("selection_left", x_min)
            .color(edge)
            .width(1.0_f32)
            .style(LineStyle::dashed_dense()),
    );
    plot_ui.vline(
        VLine::new("selection_right", x_max)
            .color(edge)
            .width(1.0_f32)
            .style(LineStyle::dashed_dense()),
    );
}

pub fn handle_marker_interaction(
    state: &mut OverlayState,
    response: &egui::Response,
    cursor_pos: Option<PlotPoint>,
) {
    let Some(pos) = cursor_pos else {
        return;
    };

    if response.double_clicked() {
        state.editing_marker = nearest_marker_index(&state.markers, pos.x, 1.0);
        return;
    }

    if response.clicked() {
        if nearest_marker_index(&state.markers, pos.x, 1.0).is_some() {
            return;
        }

        state.markers.push(UserMarker {
            x: pos.x,
            label: format!("M{}", state.markers.len() + 1),
        });
    }
}

pub fn draw_markers(
    plot_ui: &mut PlotUi<'_>,
    user_markers: &[UserMarker],
    mode_changes: &[ModeChangeMarker],
) {
    let bounds = plot_ui.plot_bounds();
    if !bounds.is_valid() {
        return;
    }

    let y_min = bounds.min()[1];
    let y_max = bounds.max()[1];
    let y_span = (y_max - y_min).abs();
    let user_label_y = y_max - y_span * 0.08;

    for (idx, marker) in mode_changes.iter().enumerate() {
        // Stagger Y position to reduce overlap when markers are close together
        let stagger_offset = (idx % 3) as f64 * y_span * 0.05;
        let mode_label_y = y_max - y_span * 0.04 - stagger_offset;

        plot_ui.vline(
            VLine::new(format!("mode_change_line_{idx}"), marker.x)
                .color(marker.color.linear_multiply(0.5))
                .style(LineStyle::dashed_dense()),
        );
        plot_ui.text(
            Text::new(
                format!("mode_change_label_{idx}"),
                PlotPoint::new(marker.x, mode_label_y),
                marker.label.clone(),
            )
            .anchor(Align2::LEFT_BOTTOM)
            .color(marker.color.linear_multiply(0.8)),
        );
    }

    for (idx, marker) in user_markers.iter().enumerate() {
        let color = Color32::from_rgb(255, 110, 110);
        plot_ui.vline(VLine::new(format!("user_marker_line_{idx}"), marker.x).color(color));
        plot_ui.text(
            Text::new(
                format!("user_marker_label_{idx}"),
                PlotPoint::new(marker.x, user_label_y),
                marker.label.clone(),
            )
            .anchor(Align2::CENTER_TOP)
            .color(color),
        );
    }
}

pub fn draw_crosshair(
    plot_ui: &mut PlotUi<'_>,
    cursor_pos: Option<PlotPoint>,
    cursor_info: Option<&CursorInfo>,
    live_now: Option<&CursorInfo>,
) {
    let Some(pos) = cursor_pos else {
        return;
    };

    let color = Color32::from_rgba_premultiplied(220, 225, 235, 70);

    plot_ui.vline(
        VLine::new("cursor_crosshair_x", pos.x)
            .color(color)
            .style(LineStyle::dotted_dense()),
    );
    plot_ui.hline(
        HLine::new("cursor_crosshair_y", pos.y)
            .color(color)
            .style(LineStyle::dotted_dense()),
    );
    if cursor_info.is_some() || live_now.is_some() {
        let mut tooltip_lines = Vec::new();
        if cursor_info.is_some() {
            tooltip_lines.push(info_bar::format_cursor_line(cursor_info));
        }
        if live_now.is_some() {
            tooltip_lines.push(info_bar::format_live_now_line(live_now));
        }
        let tooltip = tooltip_lines.join("\n");
        plot_ui.text(
            Text::new("cursor_tooltip", PlotPoint::new(pos.x, pos.y), tooltip)
                .anchor(Align2::LEFT_BOTTOM)
                .color(Color32::WHITE),
        );
    }
}

pub fn show_marker_edit_popup(ctx: &egui::Context, state: &mut OverlayState) {
    let Some(idx) = state.editing_marker else {
        return;
    };

    if idx >= state.markers.len() {
        state.editing_marker = None;
        return;
    }

    let mut open = true;
    let mut close_popup = false;
    let mut delete_marker = false;

    egui::Window::new("Edit Marker")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Label:");
                ui.text_edit_singleline(&mut state.markers[idx].label);
            });
            ui.horizontal(|ui| {
                if ui.button("Done").clicked() {
                    close_popup = true;
                }
                if ui.button("Delete").clicked() {
                    delete_marker = true;
                }
            });
        });

    if delete_marker {
        state.markers.remove(idx);
        state.editing_marker = None;
    } else if close_popup || !open {
        state.editing_marker = None;
    }
}

fn nearest_marker_index(markers: &[UserMarker], x: f64, threshold: f64) -> Option<usize> {
    markers
        .iter()
        .enumerate()
        .map(|(idx, marker)| (idx, (marker.x - x).abs()))
        .filter(|(_, distance)| *distance <= threshold)
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(idx, _)| idx)
}

fn ordered_range(start: f64, end: f64) -> (f64, f64) {
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

fn format_duration(seconds: f64) -> String {
    if seconds >= 3600.0 {
        let hours = (seconds / 3600.0).floor() as u64;
        let minutes = ((seconds % 3600.0) / 60.0).floor() as u64;
        let remainder = seconds % 60.0;
        format!("{hours}h {minutes}m {remainder:.1}s")
    } else if seconds >= 60.0 {
        let minutes = (seconds / 60.0).floor() as u64;
        let remainder = seconds % 60.0;
        format!("{minutes}m {remainder:.1}s")
    } else if seconds >= 1.0 {
        format!("{seconds:.3}s")
    } else {
        format!("{:.0}ms", seconds * 1_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::compute_selection_stats;
    use readout_core::csv_record::CsvRecord;

    #[test]
    fn compute_selection_stats_ignores_none_values_within_selected_x_range() {
        let records = vec![
            CsvRecord {
                timestamp: "2026-03-29T10:00:00Z".to_string(),
                parsed_time: Some(8.0),
                device: "Multimeter".to_string(),
                value: Some(1.0),
                unit: "V".to_string(),
                mode: "DCV".to_string(),
                is_overload: false,
                is_open: false,
                is_short: false,
            },
            CsvRecord {
                timestamp: "2026-03-29T10:00:01Z".to_string(),
                parsed_time: Some(9.0),
                device: "Multimeter".to_string(),
                value: None,
                unit: "V".to_string(),
                mode: "DCV".to_string(),
                is_overload: true,
                is_open: false,
                is_short: false,
            },
            CsvRecord {
                timestamp: "2026-03-29T10:00:02Z".to_string(),
                parsed_time: Some(10.0),
                device: "Multimeter".to_string(),
                value: Some(4.0),
                unit: "V".to_string(),
                mode: "DCV".to_string(),
                is_overload: false,
                is_open: false,
                is_short: false,
            },
            CsvRecord {
                timestamp: "2026-03-29T10:00:03Z".to_string(),
                parsed_time: Some(11.0),
                device: "Multimeter".to_string(),
                value: Some(100.0),
                unit: "V".to_string(),
                mode: "DCV".to_string(),
                is_overload: false,
                is_open: false,
                is_short: false,
            },
        ];

        let stats =
            compute_selection_stats(&records, 9.5, 10.5).expect("stats for selected values");

        assert_eq!(stats.min, 4.0);
        assert_eq!(stats.max, 4.0);
        assert_eq!(stats.avg, 4.0);
        assert_eq!(stats.stddev, 0.0);
    }
}
