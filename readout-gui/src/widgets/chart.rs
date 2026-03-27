use crate::theme::{self, colors};
use readout_core::chart_pipeline::ChartPipeline;
use readout_core::dashboard_state::{UsbCMetric, USBC_METRICS};
use readout_core::types::DeviceId;
use std::collections::HashMap;
use std::time::Duration;

pub const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

pub struct ChartState {
    pub selected_range_idx: usize,
    pub usbc_metric: UsbCMetric,
    pub split_charts: bool,
}

impl Default for ChartState {
    fn default() -> Self {
        Self {
            selected_range_idx: 0,
            usbc_metric: UsbCMetric::Voltage,
            split_charts: false,
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    pipelines: &mut HashMap<DeviceId, ChartPipeline>,
    usbc_pipelines: &mut HashMap<UsbCMetric, ChartPipeline>,
    chart_state: &mut ChartState,
) {
    // Controls row
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Range:")
                .size(11.0)
                .color(theme::text_secondary(ui)),
        );
        for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
            let selected = i == chart_state.selected_range_idx;
            if ui.selectable_label(selected, *label).clicked() {
                chart_state.selected_range_idx = i;
            }
        }

        ui.separator();

        ui.label(
            egui::RichText::new("USB-C:")
                .size(11.0)
                .color(theme::text_secondary(ui)),
        );
        for (metric, label) in USBC_METRICS {
            let selected = chart_state.usbc_metric == *metric;
            if ui.selectable_label(selected, *label).clicked() {
                chart_state.usbc_metric = *metric;
            }
        }

        ui.separator();
        if ui
            .selectable_label(chart_state.split_charts, "Split")
            .clicked()
        {
            chart_state.split_charts = !chart_state.split_charts;
        }
    });

    let (range, _) = RANGE_OPTIONS[chart_state.selected_range_idx];
    let target_points = (ui.available_width() as usize).max(100);

    let now = pipelines
        .values()
        .filter_map(|p| p.latest_timestamp())
        .chain(usbc_pipelines.values().filter_map(|p| p.latest_timestamp()))
        .max()
        .unwrap_or(Duration::ZERO);

    let mm_data: Vec<[f64; 2]> = pipelines
        .get_mut(&DeviceId::Multimeter)
        .map(|p| {
            p.query_with_now(range, target_points, now)
                .iter()
                .map(|(t, v)| [t.as_secs_f64(), *v])
                .collect()
        })
        .unwrap_or_default();

    let usbc_data: Vec<[f64; 2]> = usbc_pipelines
        .get_mut(&chart_state.usbc_metric)
        .map(|p| {
            p.query_with_now(range, target_points, now)
                .iter()
                .map(|(t, v)| [t.as_secs_f64(), *v])
                .collect()
        })
        .unwrap_or_default();

    let usbc_label = USBC_METRICS
        .iter()
        .find(|(m, _)| *m == chart_state.usbc_metric)
        .map(|(_, l)| format!("USB-C ({l})"))
        .unwrap_or_else(|| "USB-C".into());

    let mm_stroke = egui::Stroke::new(2.0, colors::MM_LINE);
    let usbc_stroke = egui::Stroke::new(2.0, colors::USBC_LINE);

    if chart_state.split_charts {
        let available_height = ui.available_height();
        let half = available_height / 2.0 - 4.0;

        egui_plot::Plot::new("mm_chart")
            .height(half)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                if !mm_data.is_empty() {
                    plot_ui.line(
                        egui_plot::Line::new("Multimeter", mm_data).stroke(mm_stroke),
                    );
                }
            });

        ui.add_space(4.0);

        egui_plot::Plot::new("usbc_chart")
            .height(half)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                if !usbc_data.is_empty() {
                    plot_ui.line(
                        egui_plot::Line::new(usbc_label, usbc_data).stroke(usbc_stroke),
                    );
                }
            });
    } else {
        egui_plot::Plot::new("main_chart")
            .height(200.0)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                if !mm_data.is_empty() {
                    plot_ui.line(
                        egui_plot::Line::new("Multimeter", mm_data).stroke(mm_stroke),
                    );
                }
                if !usbc_data.is_empty() {
                    plot_ui.line(
                        egui_plot::Line::new(usbc_label, usbc_data).stroke(usbc_stroke),
                    );
                }
            });
    }
}
