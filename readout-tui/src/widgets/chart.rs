use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::text::Line as TextLine;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use readout_core::chart_pipeline::ChartPipeline;
use readout_core::dashboard_state::{UsbCMetric, USBC_METRICS};
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
    pub usbc_metric: UsbCMetric,
    mm_buf: Vec<(f64, f64)>,
    usbc_buf: Vec<(f64, f64)>,
}

impl Default for TuiChartState {
    fn default() -> Self {
        Self {
            selected_range_idx: 0,
            usbc_metric: UsbCMetric::Voltage,
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

    pub fn next_usbc_metric(&mut self) {
        let idx = USBC_METRICS
            .iter()
            .position(|(m, _)| *m == self.usbc_metric)
            .unwrap_or(0);
        let next = (idx + 1) % USBC_METRICS.len();
        self.usbc_metric = USBC_METRICS[next].0;
    }

    pub fn range_label(&self) -> &str {
        RANGE_OPTIONS[self.selected_range_idx].1
    }

    pub fn usbc_metric_label(&self) -> &str {
        USBC_METRICS
            .iter()
            .find(|(m, _)| *m == self.usbc_metric)
            .map(|(_, l)| *l)
            .unwrap_or("V")
    }
}

fn y_bounds(buf: &[(f64, f64)]) -> [f64; 2] {
    let (lo, hi) = buf
        .iter()
        .map(|(_, v)| *v)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    if lo > hi {
        [0.0, 1.0]
    } else if (hi - lo).abs() < 0.001 {
        [lo - 0.5, hi + 0.5]
    } else {
        let margin = (hi - lo) * 0.1;
        [lo - margin, hi + margin]
    }
}

fn x_bounds(buf: &[(f64, f64)]) -> [f64; 2] {
    let (t_lo, t_hi) = buf
        .iter()
        .map(|(t, _)| *t)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(t), hi.max(t))
        });
    if t_lo > t_hi {
        [0.0, 1.0]
    } else {
        [t_lo, t_hi.max(t_lo + 1.0)]
    }
}

/// Render a chart using Canvas with Bresenham line drawing (no scatter dots).
fn render_canvas_chart(
    frame: &mut Frame,
    area: Rect,
    buf: &[(f64, f64)],
    title: &str,
    color: Color,
) {
    if area.height < 3 || area.width < 10 {
        return;
    }

    let y = y_bounds(buf);
    let x = x_bounds(buf);

    // Layout: [y-axis labels (6 chars)] [canvas]
    let chunks = Layout::horizontal([Constraint::Length(7), Constraint::Min(10)]).split(area);

    // Y-axis labels
    let y_labels = Paragraph::new(vec![
        TextLine::from(format!("{:>6.2}", y[1])),
        // Fill middle with empty lines
        TextLine::from(""),
        TextLine::from(format!("{:>6.2}", y[0])),
    ])
    .style(Style::default().fg(Color::DarkGray));

    // Position labels at top and bottom of chart area
    let label_area = chunks[0];
    if label_area.height >= 3 {
        let top_label = Rect::new(label_area.x, label_area.y + 1, label_area.width, 1);
        let bottom_label = Rect::new(
            label_area.x,
            label_area.y + label_area.height.saturating_sub(2),
            label_area.width,
            1,
        );
        frame.render_widget(
            Paragraph::new(format!("{:>6.2}", y[1])).style(Style::default().fg(Color::DarkGray)),
            top_label,
        );
        frame.render_widget(
            Paragraph::new(format!("{:>6.2}", y[0])).style(Style::default().fg(Color::DarkGray)),
            bottom_label,
        );
    }

    // Canvas with Bresenham lines
    let canvas = Canvas::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title.to_string())
                .title_style(Style::default().fg(Color::White)),
        )
        .marker(Marker::Braille)
        .x_bounds(x)
        .y_bounds(y)
        .paint(move |ctx| {
            // Draw lines between consecutive points (Bresenham)
            for pair in buf.windows(2) {
                ctx.draw(&CanvasLine {
                    x1: pair[0].0,
                    y1: pair[0].1,
                    x2: pair[1].0,
                    y2: pair[1].1,
                    color,
                });
            }
        });

    frame.render_widget(canvas, chunks[1]);
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    pipelines: &mut HashMap<DeviceId, ChartPipeline>,
    usbc_pipelines: &mut HashMap<UsbCMetric, ChartPipeline>,
    chart_state: &mut TuiChartState,
) {
    let (range, _) = RANGE_OPTIONS[chart_state.selected_range_idx];
    let target_points = (area.width as usize) * 2;

    let now = pipelines
        .values()
        .filter_map(|p| p.latest_timestamp())
        .chain(usbc_pipelines.values().filter_map(|p| p.latest_timestamp()))
        .max()
        .unwrap_or(Duration::ZERO);

    chart_state.mm_buf.clear();
    if let Some(pipeline) = pipelines.get_mut(&DeviceId::Multimeter) {
        let points = pipeline.query_smooth(range, target_points, now);
        chart_state
            .mm_buf
            .extend(points.iter().map(|(t, v)| (t.as_secs_f64(), *v)));
    }

    chart_state.usbc_buf.clear();
    if let Some(pipeline) = usbc_pipelines.get_mut(&chart_state.usbc_metric) {
        let points = pipeline.query_smooth(range, target_points, now);
        chart_state
            .usbc_buf
            .extend(points.iter().map(|(t, v)| (t.as_secs_f64(), *v)));
    }

    let range_label = chart_state.range_label();
    let metric_label = chart_state.usbc_metric_label();

    let chunks =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    render_canvas_chart(
        frame,
        chunks[0],
        &chart_state.mm_buf,
        &format!(" Multimeter [{range_label}] "),
        Color::Cyan,
    );

    render_canvas_chart(
        frame,
        chunks[1],
        &chart_state.usbc_buf,
        &format!(" USB-C [{metric_label}] [{range_label}] "),
        Color::Yellow,
    );
}
