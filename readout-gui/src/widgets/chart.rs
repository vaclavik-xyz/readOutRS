use readout_core::chart_pipeline::ChartPipeline;
use readout_core::types::DeviceId;
use std::time::Duration;

const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

pub struct ChartState {
    pub selected_range_idx: usize,
    mm_buf: Vec<[f64; 2]>,
    usbc_buf: Vec<[f64; 2]>,
}

impl Default for ChartState {
    fn default() -> Self {
        Self {
            selected_range_idx: 0,
            mm_buf: Vec::new(),
            usbc_buf: Vec::new(),
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    pipelines: &mut std::collections::HashMap<DeviceId, ChartPipeline>,
    chart_state: &mut ChartState,
) {
    // Range picker
    ui.horizontal(|ui| {
        ui.label("Range:");
        for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
            let selected = i == chart_state.selected_range_idx;
            if ui.selectable_label(selected, *label).clicked() {
                chart_state.selected_range_idx = i;
            }
        }
    });

    let (range, _) = RANGE_OPTIONS[chart_state.selected_range_idx];
    let target_points = (ui.available_width() as usize).max(100);

    // Reuse buffers to avoid per-frame allocations
    chart_state.mm_buf.clear();
    if let Some(pipeline) = pipelines.get_mut(&DeviceId::Multimeter) {
        let points = pipeline.query(range, target_points);
        chart_state
            .mm_buf
            .extend(points.iter().map(|(t, v)| [t.as_secs_f64(), *v]));
    }

    chart_state.usbc_buf.clear();
    if let Some(pipeline) = pipelines.get_mut(&DeviceId::UsbC) {
        let points = pipeline.query(range, target_points);
        chart_state
            .usbc_buf
            .extend(points.iter().map(|(t, v)| [t.as_secs_f64(), *v]));
    }

    egui_plot::Plot::new("main_chart")
        .height(200.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            if !chart_state.mm_buf.is_empty() {
                plot_ui.line(
                    egui_plot::Line::new("Multimeter", chart_state.mm_buf.clone())
                        .stroke(egui::Stroke::new(
                            1.5,
                            egui::Color32::from_rgb(100, 180, 255),
                        )),
                );
            }
            if !chart_state.usbc_buf.is_empty() {
                plot_ui.line(
                    egui_plot::Line::new("USB-C", chart_state.usbc_buf.clone())
                        .stroke(egui::Stroke::new(
                            1.5,
                            egui::Color32::from_rgb(255, 160, 80),
                        )),
                );
            }
        });
}
