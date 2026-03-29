pub struct CursorInfo {
    pub value: f64,
    pub unit: String,
    pub timestamp: String,
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

pub fn show(
    ui: &mut egui::Ui,
    cursor: Option<&CursorInfo>,
    stats: Option<&SelectionStats>,
    delta: Option<&MeasurementDelta>,
) {
    ui.horizontal_wrapped(|ui| {
        match cursor {
            Some(cursor) => {
                ui.label(
                    egui::RichText::new(format!(
                        "Cursor {:.4} {} @ {}",
                        cursor.value, cursor.unit, cursor.timestamp
                    ))
                    .small(),
                );
            }
            None => {
                ui.label(egui::RichText::new("Cursor -").small().weak());
            }
        }

        ui.separator();

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
                ui.label(egui::RichText::new("Selection stats pending").small().weak());
            }
        }

        ui.separator();

        match delta {
            Some(delta) => {
                ui.label(
                    egui::RichText::new(format!("Δt {}  Δv {:.4}", delta.dt, delta.dv)).small(),
                );
            }
            None => {
                ui.label(egui::RichText::new("Delta -").small().weak());
            }
        }
    });
}
