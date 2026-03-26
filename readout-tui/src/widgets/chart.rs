use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType};
use ratatui::Frame;

use readout_core::chart_pipeline::ChartPipeline;
use readout_core::types::DeviceId;
use std::collections::HashMap;
use std::time::Duration;

const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

pub struct TuiChartState {
    pub selected_range_idx: usize,
    mm_buf: Vec<(f64, f64)>,
    usbc_buf: Vec<(f64, f64)>,
}

impl Default for TuiChartState {
    fn default() -> Self {
        Self {
            selected_range_idx: 0,
            mm_buf: Vec::new(),
            usbc_buf: Vec::new(),
        }
    }
}

impl TuiChartState {
    pub fn next_range(&mut self) {
        self.selected_range_idx = (self.selected_range_idx + 1) % RANGE_OPTIONS.len();
    }

    pub fn prev_range(&mut self) {
        if self.selected_range_idx == 0 {
            self.selected_range_idx = RANGE_OPTIONS.len() - 1;
        } else {
            self.selected_range_idx -= 1;
        }
    }

    pub fn range_label(&self) -> &str {
        RANGE_OPTIONS[self.selected_range_idx].1
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    pipelines: &mut HashMap<DeviceId, ChartPipeline>,
    chart_state: &mut TuiChartState,
) {
    let (range, _) = RANGE_OPTIONS[chart_state.selected_range_idx];
    let target_points = area.width as usize;

    // Update buffers
    chart_state.mm_buf.clear();
    if let Some(pipeline) = pipelines.get_mut(&DeviceId::Multimeter) {
        let points = pipeline.query(range, target_points);
        chart_state
            .mm_buf
            .extend(points.iter().map(|(t, v)| (t.as_secs_f64(), *v)));
    }

    chart_state.usbc_buf.clear();
    if let Some(pipeline) = pipelines.get_mut(&DeviceId::UsbC) {
        let points = pipeline.query(range, target_points);
        chart_state
            .usbc_buf
            .extend(points.iter().map(|(t, v)| (t.as_secs_f64(), *v)));
    }

    let mut datasets = Vec::new();
    if !chart_state.mm_buf.is_empty() {
        datasets.push(
            Dataset::default()
                .name("MM")
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(&chart_state.mm_buf),
        );
    }
    if !chart_state.usbc_buf.is_empty() {
        datasets.push(
            Dataset::default()
                .name("USB-C")
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&chart_state.usbc_buf),
        );
    }

    // Auto-scale Y axis
    let all_values: Vec<f64> = chart_state
        .mm_buf
        .iter()
        .chain(chart_state.usbc_buf.iter())
        .map(|(_, v)| *v)
        .collect();

    let (y_min, y_max) = if all_values.is_empty() {
        (0.0, 1.0)
    } else {
        let min = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let margin = (max - min).max(0.1) * 0.1;
        (min - margin, max + margin)
    };

    let x_bounds = if chart_state.mm_buf.is_empty() && chart_state.usbc_buf.is_empty() {
        [0.0, 1.0]
    } else {
        let all_times: Vec<f64> = chart_state
            .mm_buf
            .iter()
            .chain(chart_state.usbc_buf.iter())
            .map(|(t, _)| *t)
            .collect();
        let t_min = all_times.iter().cloned().fold(f64::INFINITY, f64::min);
        let t_max = all_times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        [t_min, t_max.max(t_min + 1.0)]
    };

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Chart [{}] ", chart_state.range_label())),
        )
        .x_axis(Axis::default().bounds(x_bounds))
        .y_axis(
            Axis::default()
                .bounds([y_min, y_max])
                .labels(vec![ratatui::text::Line::from(format!("{y_min:.1}")), ratatui::text::Line::from(format!("{y_max:.1}"))]),
        );

    frame.render_widget(chart, area);
}
