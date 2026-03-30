pub struct CursorInfo {
    pub series: String,
    pub value: f64,
    pub unit: String,
    pub timestamp: String,
    pub mode: String,
}

pub struct SelectionStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub stddev: f64,
}

pub struct MeasurementDelta {
    pub dt: String,
    pub dv: f64,
}

pub fn format_cursor_line(cursor: Option<&CursorInfo>) -> String {
    format_sample_line("Cursor", cursor)
}

pub fn format_live_now_line(cursor: Option<&CursorInfo>) -> String {
    format_sample_line("Live now", cursor)
}

fn format_sample_line(label: &str, cursor: Option<&CursorInfo>) -> String {
    match cursor {
        Some(cursor) => format!(
            "{label} {} {:.4} {} @ {} [{}]",
            cursor.series, cursor.value, cursor.unit, cursor.timestamp, cursor.mode
        ),
        None => format!("{label} -"),
    }
}

pub fn show(
    ui: &mut egui::Ui,
    cursor: Option<&CursorInfo>,
    live_now: Option<&CursorInfo>,
    stats: Option<&SelectionStats>,
    delta: Option<&MeasurementDelta>,
) {
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            let cursor_line = format_cursor_line(cursor);
            if cursor.is_some() {
                ui.label(egui::RichText::new(cursor_line).small());
            } else {
                ui.label(egui::RichText::new(cursor_line).small().weak());
            }

            ui.separator();

            let live_now_line = format_live_now_line(live_now);
            if live_now.is_some() {
                ui.label(egui::RichText::new(live_now_line).small());
            } else {
                ui.label(egui::RichText::new(live_now_line).small().weak());
            }
        });

        ui.horizontal_wrapped(|ui| {
            match stats {
                Some(stats) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "Min {:.4}  Max {:.4}  Avg {:.4}  σ {:.4}",
                            stats.min, stats.max, stats.avg, stats.stddev
                        ))
                        .small(),
                    );
                }
                None => {
                    ui.label(egui::RichText::new("Selection -").small().weak());
                }
            }

            ui.separator();

            match delta {
                Some(delta) => {
                    ui.label(
                        egui::RichText::new(format!("Δt {}  Δv {:+.4}", delta.dt, delta.dv))
                            .small(),
                    );
                }
                None => {
                    ui.label(egui::RichText::new("Measure -").small().weak());
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{CursorInfo, format_cursor_line, format_live_now_line};

    fn sample_cursor() -> CursorInfo {
        CursorInfo {
            series: "MM Live".to_owned(),
            value: 12.0791,
            unit: "VDC".to_owned(),
            timestamp: "2026-03-30T00:29:21Z".to_owned(),
            mode: "DC".to_owned(),
        }
    }

    #[test]
    fn graph_viewer_cursor_live_cursor_line_is_labeled() {
        assert_eq!(
            format_cursor_line(Some(&sample_cursor())),
            "Cursor MM Live 12.0791 VDC @ 2026-03-30T00:29:21Z [DC]"
        );
        assert_eq!(format_cursor_line(None), "Cursor -");
    }

    #[test]
    fn graph_viewer_cursor_live_now_line_is_labeled() {
        assert_eq!(
            format_live_now_line(Some(&sample_cursor())),
            "Live now MM Live 12.0791 VDC @ 2026-03-30T00:29:21Z [DC]"
        );
        assert_eq!(format_live_now_line(None), "Live now -");
    }
}
