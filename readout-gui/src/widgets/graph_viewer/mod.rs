mod data_store;
mod info_bar;
mod overlay;
mod source_model;
mod viewer_toolbar;

use self::data_store::CsvDataStore;
use self::overlay::ModeChangeMarker;
use self::source_model::XDomain;
use chrono::{DateTime, Local};
use readout_core::types::{DeviceId, RuntimeEvent};
use readout_persistence::config::AppConfiguration;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(200);
pub const GRAPH_VIEWER_WINDOW_TITLE: &str = "Graph Viewer";
pub const GRAPH_VIEWER_VIEWPORT_ID: &str = "graph_viewer";
pub const GRAPH_VIEWER_PLOT_ID: &str = "graph_viewer_plot";

pub struct GraphViewerWindow {
    pub open: bool,
    data_store: CsvDataStore,
    interaction_mode: InteractionMode,
    following: bool,
    snap_follow_next_frame: bool,
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
    AttachRuntime(DeviceId),
    AttachLiveCsv(DeviceId),
    ZoomFit,
    SetMode(InteractionMode),
    Export,
    ToggleFollow,
    ToggleSourceVisibility(u64),
    RemoveSource(u64),
}

impl GraphViewerWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            data_store: CsvDataStore::new(),
            interaction_mode: InteractionMode::Normal,
            following: true,
            snap_follow_next_frame: false,
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
            .with_title(GRAPH_VIEWER_WINDOW_TITLE)
            .with_inner_size([900.0, 560.0])
            .with_min_inner_size([480.0, 320.0]);
        if config.always_on_top {
            viewport = viewport.with_always_on_top();
        }

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(GRAPH_VIEWER_VIEWPORT_ID),
            viewport,
            |ctx, _class| {
                let has_live_files = self.data_store.files().iter().any(|file| file.is_live);
                self.handle_keyboard_shortcuts(ctx, config);

                if has_live_files && self.last_poll.elapsed() >= LIVE_POLL_INTERVAL {
                    self.data_store.poll_live_sources();
                    self.last_poll = Instant::now();
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    let action = viewer_toolbar::show(
                        ui,
                        &mut self.data_store,
                        self.interaction_mode,
                        self.following,
                    );
                    self.handle_action(action, config);

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
                    ctx.request_repaint_after(LIVE_POLL_INTERVAL);
                }

                if ctx.input(|i| i.viewport().close_requested()) {
                    self.open = false;
                }
            },
        );
    }

    fn handle_action(&mut self, action: ViewerAction, config: &AppConfiguration) {
        match action {
            ViewerAction::OpenFile => {
                if let Some(path) = pick_csv_file() {
                    match self.data_store.load_csv_file(path, true) {
                        Ok(_) => {
                            self.overlay = overlay::OverlayState::default();
                            self.hovered_cursor = None;
                            self.following = false;
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
                    match self.data_store.load_csv_file(path, false) {
                        Ok(_) => {
                            self.fit_next_frame = true;
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_error = Some(format!("Failed to add CSV: {err}"));
                        }
                    }
                }
            }
            ViewerAction::AttachRuntime(device) => match self.data_store.attach_runtime_device(device) {
                        Ok(_) => {
                            self.following = true;
                            self.snap_follow_next_frame = true;
                            self.last_error = None;
                        }
                Err(err) => {
                    self.last_error = Some(format!("Failed to attach live source: {err}"));
                }
            },
            ViewerAction::AttachLiveCsv(device) => {
                let Some(path) = configured_tail_path(config, device) else {
                    self.last_error = Some(format!(
                        "No CSV log path configured for {}",
                        match device {
                            DeviceId::Multimeter => "Multimeter",
                            DeviceId::UsbC => "USB-C",
                        }
                    ));
                    return;
                };

                match self.data_store.attach_live_csv(device, path) {
                    Ok(_) => {
                        self.following = true;
                        self.snap_follow_next_frame = true;
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_error = Some(format!("Failed to attach CSV tail: {err}"));
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
                self.snap_follow_next_frame = self.following;
            }
            ViewerAction::ToggleSourceVisibility(source_id) => {
                if let Some(file) = self
                    .data_store
                    .files_mut()
                    .iter_mut()
                    .find(|file| file.id == source_id)
                {
                    file.visible = !file.visible;
                }
                self.hovered_cursor = None;
            }
            ViewerAction::RemoveSource(source_id) => {
                self.data_store.remove_source(source_id);
                if self.data_store.file_count() == 0 {
                    self.overlay = overlay::OverlayState::default();
                    self.following = false;
                }
                self.hovered_cursor = None;
            }
            ViewerAction::None => {}
        }
    }

    pub fn handle_runtime_event(&mut self, event: &RuntimeEvent) {
        self.data_store.handle_runtime_event(event);
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
        let active_domain = self.data_store.active_domain();
        let fit_next_frame = self.fit_next_frame;
        let snap_follow_next_frame = self.snap_follow_next_frame;
        let mut consumed_follow_snap = false;
        let plot_response = egui_plot::Plot::new(GRAPH_VIEWER_PLOT_ID)
            .allow_zoom(true)
            .allow_drag(self.interaction_mode == InteractionMode::Normal)
            .allow_scroll(true)
            .x_axis_formatter(move |mark, _range| format_axis_label(mark.value, active_domain))
            .label_formatter(|_, _| String::new())
            .show(ui, |plot_ui| {
                let cursor_pos = if plot_ui.response().hovered() || plot_ui.response().dragged() {
                    plot_ui.pointer_coordinate()
                } else {
                    None
                };
                self.overlay.cursor_pos = cursor_pos;
                self.hovered_cursor =
                    cursor_pos.and_then(|pos| self.cursor_info_for_point(pos.x, pos.y));

                for file in self.data_store.files() {
                    if !file.visible {
                        continue;
                    }

                    let points = self.data_store.query_points(file.id, 2_000);
                    if points.is_empty() {
                        continue;
                    }

                    let series: Vec<[f64; 2]> = points
                        .into_iter()
                        .map(|(time, value)| [time.as_secs_f64(), value])
                        .collect();
                    plot_ui.line(
                        egui_plot::Line::new(file.label.clone(), series)
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
                    consumed_follow_snap =
                        follow_live_edge(plot_ui, latest_x, snap_follow_next_frame);
                }
            });
        self.fit_next_frame = false;
        if consumed_follow_snap {
            self.snap_follow_next_frame = false;
        }

        if self.following
            && should_detach_follow(
                ui,
                &plot_response.response,
                self.interaction_mode == InteractionMode::Normal,
            )
        {
            self.following = false;
            self.snap_follow_next_frame = false;
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

    fn cursor_info_for_point(&self, x: f64, y: f64) -> Option<info_bar::CursorInfo> {
        let record = self.data_store.nearest_visible_sample(x, y)?;

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
                    let x = file
                        .samples
                        .get(*idx)
                        .map(|sample| sample.x)
                        .unwrap_or_else(|| data_store::record_x(record, *idx));

                    Some(ModeChangeMarker {
                        x,
                        label: format!("{} → {}", previous.mode, record.mode),
                        color: file.color,
                    })
                })
            })
            .collect()
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, config: &AppConfiguration) {
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
            self.handle_action(ViewerAction::AddFile, config);
            return;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.handle_action(ViewerAction::OpenFile, config);
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

fn configured_tail_path(config: &AppConfiguration, device: DeviceId) -> Option<PathBuf> {
    let path = match device {
        DeviceId::Multimeter => &config.multimeter_csv_log_file_path,
        DeviceId::UsbC => &config.usbc_csv_log_file_path,
    };

    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
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

fn follow_live_edge(
    plot_ui: &mut egui_plot::PlotUi<'_>,
    latest_x: f64,
    force_snap: bool,
) -> bool {
    let bounds = plot_ui.plot_bounds();
    if !bounds.is_valid() {
        return false;
    }

    let Some((x_min, x_max)) =
        compute_follow_window(bounds.min()[0], bounds.max()[0], latest_x, force_snap)
    else {
        return false;
    };

    plot_ui.set_plot_bounds_x(x_min..=x_max);
    true
}

fn compute_follow_window(
    x_min: f64,
    x_max: f64,
    latest_x: f64,
    force_snap: bool,
) -> Option<(f64, f64)> {
    let width = x_max - x_min;
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    if !force_snap && latest_x <= x_max {
        return None;
    }

    Some((latest_x - width, latest_x))
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

    for row in data_store.export_rows(selection) {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{}",
            row.timestamp,
            row.device,
            row.value,
            row.unit,
            row.mode,
            row.is_overload,
            row.is_open,
            row.is_short
        )?;
    }

    Ok(())
}

fn format_axis_label(value: f64, active_domain: Option<XDomain>) -> String {
    match active_domain.unwrap_or(XDomain::WallClock) {
        XDomain::WallClock => format_epoch_axis(value),
        XDomain::SequenceIndex => format!("#{}", value.round() as i64),
    }
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

#[cfg(test)]
fn format_csv_value(value: Option<f64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "OL".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CsvDataStore, GRAPH_VIEWER_PLOT_ID, GRAPH_VIEWER_VIEWPORT_ID, GRAPH_VIEWER_WINDOW_TITLE,
        GraphViewerWindow, ViewerAction, export_to_csv, format_csv_value,
    };
    use readout_core::types::DeviceId;
    use readout_persistence::config::AppConfiguration;
    use std::fs;
    use std::time::{Duration, Instant};

    #[test]
    fn toggle_follow_does_not_reset_poll_timer() {
        let mut viewer = GraphViewerWindow::new();
        let config = AppConfiguration::default();
        let last_poll = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .expect("valid instant subtraction");
        viewer.last_poll = last_poll;

        viewer.handle_action(ViewerAction::ToggleFollow, &config);

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

    #[test]
    fn attach_live_csv_without_configured_path_sets_error() {
        let mut viewer = GraphViewerWindow::new();
        let config = AppConfiguration::default();

        viewer.handle_action(ViewerAction::AttachLiveCsv(DeviceId::Multimeter), &config);

        assert!(
            viewer
                .last_error
                .as_deref()
                .unwrap()
                .contains("CSV log path")
        );
    }

    #[test]
    fn attach_runtime_action_creates_waiting_source() {
        let mut viewer = GraphViewerWindow::new();
        let config = AppConfiguration::default();

        viewer.handle_action(ViewerAction::AttachRuntime(DeviceId::Multimeter), &config);

        assert_eq!(viewer.data_store.sources().len(), 1);
    }

    #[test]
    fn enabling_follow_requests_live_snap_even_when_latest_point_is_inside_bounds() {
        let next = super::compute_follow_window(100.0, 200.0, 150.0, true).unwrap();
        assert_eq!(next, (50.0, 150.0));
    }

    #[test]
    fn follow_window_returns_none_without_positive_width() {
        assert!(super::compute_follow_window(100.0, 100.0, 150.0, true).is_none());
    }

    #[test]
    fn export_to_csv_includes_runtime_samples_and_selection_filter() {
        let export_path = std::env::temp_dir().join(format!(
            "readout_csv_viewer_runtime_export_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));

        let mut store = CsvDataStore::new();
        let source_id = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
        store.push_test_sample(source_id, 100.0, Some(1.0), "V", "DCV");
        store.push_test_sample(source_id, 101.0, Some(2.0), "V", "DCV");

        export_to_csv(&export_path, &store, Some((100.5, 101.5))).expect("export runtime csv");

        let exported = fs::read_to_string(&export_path).expect("read export");
        let lines: Vec<&str> = exported.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains(",2,V,DCV,"));

        fs::remove_file(&export_path).expect("remove export csv");
    }

    #[test]
    fn graph_viewer_constants_use_renamed_feature_ids() {
        assert_eq!(GRAPH_VIEWER_WINDOW_TITLE, "Graph Viewer");
        assert_eq!(GRAPH_VIEWER_VIEWPORT_ID, "graph_viewer");
        assert_eq!(GRAPH_VIEWER_PLOT_ID, "graph_viewer_plot");
    }
}
