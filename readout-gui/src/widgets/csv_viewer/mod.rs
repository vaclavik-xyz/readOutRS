mod data_store;
mod info_bar;
mod overlay;
mod viewer_toolbar;

use self::data_store::CsvDataStore;
use self::overlay::ModeChangeMarker;
use chrono::{DateTime, Local};
use readout_persistence::config::AppConfiguration;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct CsvViewerWindow {
    pub open: bool,
    data_store: CsvDataStore,
    interaction_mode: InteractionMode,
    following: bool,
    last_poll: Instant,
    overlay: overlay::OverlayState,
    hovered_cursor: Option<info_bar::CursorInfo>,
    fit_next_frame: bool,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractionMode {
    #[default]
    Normal,
    Measure,
    Select,
    Marker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewerAction {
    #[default]
    None,
    OpenFile,
    AddFile,
    ZoomFit,
    SetMode(InteractionMode),
    Export,
    ToggleFollow,
    ToggleFileVisibility(usize),
    RemoveFile(usize),
}

impl CsvViewerWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            data_store: CsvDataStore::new(),
            interaction_mode: InteractionMode::Normal,
            following: true,
            last_poll: Instant::now(),
            overlay: overlay::OverlayState::default(),
            hovered_cursor: None,
            fit_next_frame: false,
            last_error: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, config: &AppConfiguration) {
        if !self.open {
            return;
        }

        let mut viewport = egui::ViewportBuilder::default()
            .with_title("CSV Viewer")
            .with_inner_size([900.0, 560.0])
            .with_min_inner_size([480.0, 320.0]);
        if config.always_on_top {
            viewport = viewport.with_always_on_top();
        }

        let live_paths = configured_live_paths(config);

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("csv_viewer"),
            viewport,
            |ctx, _class| {
                self.data_store.sync_live_paths(&live_paths);
                let has_live_files = self.data_store.files().iter().any(|file| file.is_live);
                self.handle_keyboard_shortcuts(ctx, &live_paths);

                if has_live_files && self.last_poll.elapsed() >= Duration::from_secs(1) {
                    self.data_store.poll_live_files();
                    self.last_poll = Instant::now();
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    let action = viewer_toolbar::show(
                        ui,
                        &mut self.data_store,
                        self.interaction_mode,
                        self.following,
                    );
                    self.handle_action(action, &live_paths);

                    if let Some(error) = &self.last_error {
                        ui.colored_label(egui::Color32::from_rgb(220, 90, 90), error);
                        ui.separator();
                    }

                    ui.separator();
                    self.render_chart(ui);
                    ui.separator();
                    let selection_stats = self.selection_stats();
                    let measurement_delta =
                        overlay::active_measurement_delta(&self.overlay, self.overlay.cursor_pos);
                    info_bar::show(
                        ui,
                        self.hovered_cursor.as_ref(),
                        selection_stats.as_ref(),
                        measurement_delta.as_ref(),
                    );
                });

                overlay::show_marker_edit_popup(ctx, &mut self.overlay);

                if has_live_files {
                    ctx.request_repaint_after(Duration::from_secs(1));
                }

                if ctx.input(|i| i.viewport().close_requested()) {
                    self.open = false;
                }
            },
        );
    }

    fn handle_action(&mut self, action: ViewerAction, live_paths: &[PathBuf]) {
        match action {
            ViewerAction::OpenFile => {
                if let Some(path) = pick_csv_file() {
                    let is_live = is_live_path(&path, live_paths);
                    let mut next_store = CsvDataStore::new();
                    match next_store.load_file(path, is_live) {
                        Ok(_) => {
                            self.data_store = next_store;
                            self.overlay = overlay::OverlayState::default();
                            self.hovered_cursor = None;
                            self.following = is_live;
                            self.fit_next_frame = true;
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_error = Some(format!("Failed to open CSV: {err}"));
                        }
                    }
                }
            }
            ViewerAction::AddFile => {
                if let Some(path) = pick_csv_file() {
                    let is_live = is_live_path(&path, live_paths);
                    match self.data_store.load_file(path, is_live) {
                        Ok(_) => {
                            self.fit_next_frame = true;
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_error = Some(format!("Failed to add CSV: {err}"));
                        }
                    }
                    if is_live {
                        self.following = true;
                    }
                }
            }
            ViewerAction::SetMode(mode) => {
                self.toggle_mode(mode);
            }
            ViewerAction::ZoomFit => {
                self.fit_next_frame = true;
            }
            ViewerAction::Export => {
                if let Some(save_path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .set_file_name("export.csv")
                    .save_file()
                {
                    match export_to_csv(&save_path, &self.data_store, self.overlay.selection) {
                        Ok(()) => {
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_error = Some(format!("Export failed: {err}"));
                        }
                    }
                }
            }
            ViewerAction::ToggleFollow => {
                self.following = !self.following;
            }
            ViewerAction::ToggleFileVisibility(file_idx) => {
                if let Some(file) = self.data_store.files_mut().get_mut(file_idx) {
                    file.visible = !file.visible;
                }
                self.hovered_cursor = None;
            }
            ViewerAction::RemoveFile(file_idx) => {
                self.data_store.remove_file(file_idx);
                if self.data_store.file_count() == 0 {
                    self.overlay = overlay::OverlayState::default();
                    self.following = false;
                }
                self.hovered_cursor = None;
            }
            ViewerAction::None => {}
        }
    }

    fn render_chart(&mut self, ui: &mut egui::Ui) {
        if self.data_store.file_count() == 0 {
            self.overlay.cursor_pos = None;
            self.hovered_cursor = None;
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Open a CSV file to inspect logged measurements.").weak(),
                );
            });
            return;
        }

        let mode_change_markers = self.mode_change_markers();
        let latest_live_x = self.data_store.latest_live_x();
        let fit_next_frame = self.fit_next_frame;
        let plot_response = egui_plot::Plot::new("csv_viewer_plot")
            .allow_zoom(true)
            .allow_drag(self.interaction_mode == InteractionMode::Normal)
            .allow_scroll(true)
            .x_axis_formatter(|mark, _range| format_epoch_axis(mark.value))
            .label_formatter(|name, point| {
                // Replace egui_plot's default "x=..., y=..." tooltip
                let time = format_epoch_full(point.x);
                if name.is_empty() {
                    format!("{time}\n{:.4}", point.y)
                } else {
                    format!("{name}\n{time}\n{:.4}", point.y)
                }
            })
            .show(ui, |plot_ui| {
                let cursor_pos = if plot_ui.response().hovered() || plot_ui.response().dragged() {
                    plot_ui.pointer_coordinate()
                } else {
                    None
                };
                self.overlay.cursor_pos = cursor_pos;
                self.hovered_cursor = cursor_pos.and_then(|pos| self.cursor_info_for_x(pos.x));

                for (file_idx, file) in self.data_store.files().iter().enumerate() {
                    if !file.visible {
                        continue;
                    }

                    let points = self.data_store.query_points(file_idx, 2_000);
                    if points.is_empty() {
                        continue;
                    }

                    let series: Vec<[f64; 2]> = points
                        .into_iter()
                        .map(|(time, value)| [time.as_secs_f64(), value])
                        .collect();
                    let name = file
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("CSV");

                    plot_ui.line(
                        egui_plot::Line::new(name, series)
                            .stroke(egui::Stroke::new(1.5, file.color)),
                    );
                }

                overlay::draw_selection(plot_ui, self.overlay.selection);
                overlay::draw_measurements(plot_ui, &self.overlay, cursor_pos);
                overlay::draw_markers(plot_ui, &self.overlay.markers, &mode_change_markers);
                overlay::draw_crosshair(plot_ui, cursor_pos, self.hovered_cursor.as_ref());

                if fit_next_frame {
                    plot_ui.set_auto_bounds(egui::Vec2b::new(true, true));
                } else if self.following
                    && let Some(latest_x) = latest_live_x
                {
                    follow_live_edge(plot_ui, latest_x);
                }
            });
        self.fit_next_frame = false;

        if self.following
            && should_detach_follow(
                ui,
                &plot_response.response,
                self.interaction_mode == InteractionMode::Normal,
            )
        {
            self.following = false;
        }

        let cursor_pos = self.overlay.cursor_pos;
        match self.interaction_mode {
            InteractionMode::Measure => {
                overlay::handle_measure_interaction(
                    &mut self.overlay,
                    &plot_response.response,
                    cursor_pos,
                );
            }
            InteractionMode::Select => {
                overlay::handle_select_interaction(
                    &mut self.overlay,
                    &plot_response.response,
                    cursor_pos,
                );
            }
            InteractionMode::Marker => {
                overlay::handle_marker_interaction(
                    &mut self.overlay,
                    &plot_response.response,
                    cursor_pos,
                );
            }
            InteractionMode::Normal => {}
        }
    }

    fn selection_stats(&self) -> Option<info_bar::SelectionStats> {
        let (x_min, x_max) = self.overlay.selection?;
        let values = self.data_store.visible_values_in_range(x_min, x_max);
        overlay::compute_value_stats(&values)
    }

    fn cursor_info_for_x(&self, x: f64) -> Option<info_bar::CursorInfo> {
        let record = self.data_store.nearest_visible_record(x)?;

        Some(info_bar::CursorInfo {
            series: record.series,
            value: record.value,
            unit: record.unit,
            timestamp: record.timestamp,
            mode: record.mode,
        })
    }

    fn mode_change_markers(&self) -> Vec<ModeChangeMarker> {
        self.data_store
            .files()
            .iter()
            .filter(|file| file.visible)
            .flat_map(|file| {
                file.mode_changes.iter().filter_map(move |idx| {
                    let record = file.records.get(*idx)?;
                    let previous = file.records.get(idx.saturating_sub(1))?;

                    Some(ModeChangeMarker {
                        x: data_store::record_x(record, *idx),
                        label: format!("{} → {}", previous.mode, record.mode),
                        color: file.color,
                    })
                })
            })
            .collect()
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, live_paths: &[PathBuf]) {
        let escape_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape_pressed {
            self.cancel_current_interaction();
        }

        if ctx.wants_keyboard_input() {
            return;
        }

        let command_shift = egui::Modifiers {
            command: true,
            shift: true,
            ..Default::default()
        };

        if ctx.input_mut(|input| input.consume_key(command_shift, egui::Key::O)) {
            self.handle_action(ViewerAction::AddFile, live_paths);
            return;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.handle_action(ViewerAction::OpenFile, live_paths);
            return;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::M)) {
            self.toggle_mode(InteractionMode::Measure);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::S)) {
            self.toggle_mode(InteractionMode::Select);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::K)) {
            self.toggle_mode(InteractionMode::Marker);
        }
    }

    fn cancel_current_interaction(&mut self) -> bool {
        if self.overlay.editing_marker.take().is_some() {
            return true;
        }
        if self.overlay.measuring_from.take().is_some() {
            return true;
        }
        if self.interaction_mode == InteractionMode::Measure
            && !self.overlay.measurements.is_empty()
        {
            self.overlay.measurements.pop();
            return true;
        }
        if self.overlay.selecting_from.take().is_some() {
            return true;
        }
        if self.overlay.selection.take().is_some() {
            return true;
        }
        if self.interaction_mode != InteractionMode::Normal {
            self.interaction_mode = InteractionMode::Normal;
            return true;
        }

        false
    }

    fn toggle_mode(&mut self, mode: InteractionMode) {
        self.interaction_mode = if self.interaction_mode == mode {
            InteractionMode::Normal
        } else {
            mode
        };
    }
}

fn configured_live_paths(config: &AppConfiguration) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if config.multimeter_csv_logging_enabled && !config.multimeter_csv_log_file_path.is_empty() {
        paths.push(PathBuf::from(&config.multimeter_csv_log_file_path));
    }
    if config.usbc_csv_logging_enabled && !config.usbc_csv_log_file_path.is_empty() {
        paths.push(PathBuf::from(&config.usbc_csv_log_file_path));
    }

    paths
}

fn is_live_path(path: &Path, live_paths: &[PathBuf]) -> bool {
    live_paths.iter().any(|live_path| live_path == path)
}

fn pick_csv_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .pick_file()
}

fn should_detach_follow(ui: &egui::Ui, response: &egui::Response, allow_drag_detach: bool) -> bool {
    let scrolled = response.hovered()
        && ui.input(|input| {
            input.smooth_scroll_delta != egui::Vec2::ZERO
                || input.raw_scroll_delta != egui::Vec2::ZERO
        });

    (allow_drag_detach && response.dragged()) || scrolled
}

fn follow_live_edge(plot_ui: &mut egui_plot::PlotUi<'_>, latest_x: f64) {
    let bounds = plot_ui.plot_bounds();
    if !bounds.is_valid() || latest_x <= bounds.max()[0] {
        return;
    }

    let width = bounds.width();
    if !width.is_finite() || width <= 0.0 {
        return;
    }

    plot_ui.set_plot_bounds_x((latest_x - width)..=latest_x);
}

fn export_to_csv(
    path: &Path,
    data_store: &CsvDataStore,
    selection: Option<(f64, f64)>,
) -> Result<(), std::io::Error> {
    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short"
    )?;

    let selection = selection.map(|(start, end)| (start.min(end), start.max(end)));

    for loaded in data_store.files() {
        if !loaded.visible {
            continue;
        }

        for (idx, record) in loaded.records.iter().enumerate() {
            if !loaded.visible_modes.contains(&record.mode) {
                continue;
            }

            let record_x = data_store::record_x(record, idx);
            if let Some((x_min, x_max)) = selection
                && (record_x < x_min || record_x > x_max)
            {
                continue;
            }

            writeln!(
                file,
                "{},{},{},{},{},{},{},{}",
                record.timestamp,
                record.device,
                format_csv_value(record.value),
                record.unit,
                record.mode,
                record.is_overload,
                record.is_open,
                record.is_short
            )?;
        }
    }

    Ok(())
}

/// Format epoch seconds for X axis tick labels. Shows HH:MM:SS for intra-day,
/// or date + time when range spans multiple days.
fn format_epoch_axis(epoch_secs: f64) -> String {
    let secs = epoch_secs.floor() as i64;
    let nanos = ((epoch_secs - secs as f64) * 1e9) as u32;
    let Some(dt) = DateTime::from_timestamp(secs, nanos) else {
        return format!("{epoch_secs:.0}");
    };
    let local: DateTime<Local> = dt.with_timezone(&Local);
    local.format("%H:%M:%S").to_string()
}

/// Format epoch seconds for tooltips and info bar — full date + time with sub-second precision.
fn format_epoch_full(epoch_secs: f64) -> String {
    let secs = epoch_secs.floor() as i64;
    let nanos = ((epoch_secs - secs as f64) * 1e9) as u32;
    let Some(dt) = DateTime::from_timestamp(secs, nanos) else {
        return format!("{epoch_secs:.3}");
    };
    let local: DateTime<Local> = dt.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

fn format_csv_value(value: Option<f64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "OL".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{CsvDataStore, CsvViewerWindow, ViewerAction, export_to_csv, format_csv_value};
    use std::fs;
    use std::time::{Duration, Instant};

    #[test]
    fn toggle_follow_does_not_reset_poll_timer() {
        let mut viewer = CsvViewerWindow::new();
        let last_poll = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .expect("valid instant subtraction");
        viewer.last_poll = last_poll;

        viewer.handle_action(ViewerAction::ToggleFollow, &[]);

        assert!(!viewer.following);
        assert_eq!(viewer.last_poll, last_poll);
    }

    #[test]
    fn format_csv_value_preserves_overload_marker() {
        assert_eq!(format_csv_value(None), "OL");
        assert_eq!(format_csv_value(Some(12.5)), "12.5");
    }

    #[test]
    fn export_to_csv_respects_selection_and_visible_modes() {
        let source_path = std::env::temp_dir().join(format!(
            "readout_csv_viewer_export_source_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));
        let export_path = std::env::temp_dir().join(format!(
            "readout_csv_viewer_export_output_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));

        fs::write(
            &source_path,
            concat!(
                "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
                "2026-03-29T10:00:00Z,Multimeter,1.25,V,DCV,false,false,false\n",
                "2026-03-29T10:00:01Z,Multimeter,OL,V,DCV,true,false,false\n",
                "2026-03-29T10:00:02Z,Multimeter,3.50,V,ACV,false,false,false\n"
            ),
        )
        .expect("write source csv");

        let mut store = CsvDataStore::new();
        store
            .load_file(source_path.clone(), false)
            .expect("load source csv");
        store.set_mode_visible("ACV", false);

        export_to_csv(
            &export_path,
            &store,
            Some((1_774_778_400.5, 1_774_778_401.5)),
        )
        .expect("export filtered csv");

        let exported = fs::read_to_string(&export_path).expect("read export");
        let lines: Vec<&str> = exported.lines().collect();
        assert_eq!(
            lines,
            vec![
                "timestamp,device,value,unit,mode,is_overload,is_open,is_short",
                "2026-03-29T10:00:01Z,Multimeter,OL,V,DCV,true,false,false"
            ]
        );

        fs::remove_file(&source_path).expect("remove source csv");
        fs::remove_file(&export_path).expect("remove export csv");
    }
}
