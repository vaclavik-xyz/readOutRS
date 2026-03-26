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
}

impl Default for ChartState {
    fn default() -> Self {
        Self {
            selected_range_idx: 0,
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

    // Use a shared "now" for both pipelines so they align on the same time window.
    let now = pipelines
        .values()
        .filter_map(|p| p.latest_timestamp())
        .max()
        .unwrap_or(Duration::ZERO);

    // Query and convert to plot format — allocates per frame, unavoidable since
    // egui_plot::Line::new requires owned Vec (PlotPoints doesn't accept slices).
    let mm_data: Vec<[f64; 2]> = pipelines
        .get_mut(&DeviceId::Multimeter)
        .map(|p| {
            p.query_with_now(range, target_points, now)
                .iter()
                .map(|(t, v)| [t.as_secs_f64(), *v])
                .collect()
        })
        .unwrap_or_default();

    let usbc_data: Vec<[f64; 2]> = pipelines
        .get_mut(&DeviceId::UsbC)
        .map(|p| {
            p.query_with_now(range, target_points, now)
                .iter()
                .map(|(t, v)| [t.as_secs_f64(), *v])
                .collect()
        })
        .unwrap_or_default();

    egui_plot::Plot::new("main_chart")
        .height(200.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            if !mm_data.is_empty() {
                plot_ui.line(
                    egui_plot::Line::new("Multimeter", mm_data)
                        .stroke(egui::Stroke::new(
                            1.5,
                            egui::Color32::from_rgb(100, 180, 255),
                        )),
                );
            }
            if !usbc_data.is_empty() {
                plot_ui.line(
                    egui_plot::Line::new("USB-C", usbc_data)
                        .stroke(egui::Stroke::new(
                            1.5,
                            egui::Color32::from_rgb(255, 160, 80),
                        )),
                );
            }
        });
}
