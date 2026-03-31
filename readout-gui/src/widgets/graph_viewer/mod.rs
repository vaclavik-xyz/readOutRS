mod data_store;
mod info_bar;
mod overlay;
mod render_cache;
mod render_sampling;
mod source_model;
mod viewer_toolbar;

use self::data_store::CsvDataStore;
use self::overlay::ModeChangeMarker;
use self::render_cache::RenderCache;
use self::render_sampling::visible_point_budget;
use self::source_model::XDomain;
use chrono::{DateTime, Local};
use egui::Pos2;
use readout_core::types::{DeviceId, RuntimeEvent};
use readout_persistence::config::AppConfiguration;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(50);
pub const GRAPH_VIEWER_WINDOW_TITLE: &str = "Graph Viewer";
pub const GRAPH_VIEWER_VIEWPORT_ID: &str = "graph_viewer";
pub const GRAPH_VIEWER_PLOT_ID: &str = "graph_viewer_plot";
const SAFE_PLOT_QUERY_BUDGET: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlotQuery {
    x_range: Option<(f64, f64)>,
    target_points: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogRequestKind {
    OpenFile,
    AddFile,
}

#[derive(Debug, Clone, PartialEq)]
enum DialogRequest {
    PickCsv { kind: DialogRequestKind },
    SaveExport { selection: Option<(f64, f64)> },
}

#[derive(Debug, Clone, PartialEq)]
enum DialogOutcome {
    PickCsv {
        kind: DialogRequestKind,
        path: Option<PathBuf>,
    },
    SaveExport {
        selection: Option<(f64, f64)>,
        path: Option<PathBuf>,
    },
}

struct PendingDialog {
    result_rx: mpsc::Receiver<DialogOutcome>,
}

pub struct GraphViewerWindow {
    pub open: bool,
    data_store: CsvDataStore,
    interaction_mode: InteractionMode,
    following: bool,
    snap_follow_next_frame: bool,
    last_poll: Instant,
    overlay: overlay::OverlayState,
    hovered_cursor: Option<info_bar::CursorInfo>,
    hovered_live_now: Option<info_bar::CursorInfo>,
    hovered_source_id: Option<u64>,
    last_hover_screen_pos: Option<Pos2>,
    fit_next_frame: bool,
    last_error: Option<String>,
    render_cache: RenderCache,
    queued_dialog_request: Option<DialogRequest>,
    pending_dialog: Option<PendingDialog>,
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
            hovered_live_now: None,
            hovered_source_id: None,
            last_hover_screen_pos: None,
            fit_next_frame: false,
            last_error: None,
            render_cache: RenderCache::new(),
            queued_dialog_request: None,
            pending_dialog: None,
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
                self.poll_dialog_outcomes(config);

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
                    self.dispatch_action(action, config);

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
                        self.hovered_live_now.as_ref(),
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
            ViewerAction::AttachRuntime(device) => {
                match self.data_store.attach_runtime_device(device) {
                    Ok(_) => {
                        self.following = true;
                        self.snap_follow_next_frame = true;
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_error = Some(format!("Failed to attach live source: {err}"));
                    }
                }
            }
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
                self.render_cache.invalidate_source(source_id);
                self.clear_hover_state();
            }
            ViewerAction::RemoveSource(source_id) => {
                self.data_store.remove_source(source_id);
                self.render_cache.invalidate_source(source_id);
                if self.data_store.file_count() == 0 {
                    self.overlay = overlay::OverlayState::default();
                    self.following = false;
                }
                self.clear_hover_state();
            }
            ViewerAction::None => {}
            ViewerAction::OpenFile | ViewerAction::AddFile | ViewerAction::Export => {}
        }
    }

    fn dispatch_action(&mut self, action: ViewerAction, config: &AppConfiguration) {
        if let Some(request) = dialog_request_for_action(action, self.overlay.selection) {
            self.queue_dialog_request(request);
            return;
        }

        self.handle_action(action, config);
    }

    fn queue_dialog_request(&mut self, request: DialogRequest) {
        if self.pending_dialog.is_some() || self.queued_dialog_request.is_some() {
            return;
        }

        self.queued_dialog_request = Some(request);
    }

    fn take_dialog_request(&mut self) -> Option<DialogRequest> {
        self.queued_dialog_request.take()
    }

    pub(crate) fn launch_queued_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.take_dialog_request() else {
            return;
        };

        self.start_dialog_request(request, ctx);
    }

    fn start_dialog_request(&mut self, request: DialogRequest, ctx: &egui::Context) {
        if self.pending_dialog.is_some() {
            return;
        }

        let result_rx = spawn_dialog_request(request, ctx.clone());
        self.pending_dialog = Some(PendingDialog { result_rx });
    }

    fn poll_dialog_outcomes(&mut self, config: &AppConfiguration) {
        let Some(pending) = self.pending_dialog.take() else {
            return;
        };

        match pending.result_rx.try_recv() {
            Ok(outcome) => self.apply_dialog_outcome(outcome, config),
            Err(mpsc::TryRecvError::Empty) => {
                self.pending_dialog = Some(pending);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.last_error = Some("File dialog failed before returning a result".to_string());
            }
        }
    }

    fn apply_dialog_outcome(&mut self, outcome: DialogOutcome, _config: &AppConfiguration) {
        match outcome {
            DialogOutcome::PickCsv {
                kind: DialogRequestKind::OpenFile,
                path: Some(path),
            } => match self.data_store.load_csv_file(path, true) {
                Ok(_) => {
                    self.overlay = overlay::OverlayState::default();
                    self.clear_hover_state();
                    self.following = false;
                    self.fit_next_frame = true;
                    self.last_error = None;
                }
                Err(err) => {
                    self.last_error = Some(format!("Failed to open CSV: {err}"));
                }
            },
            DialogOutcome::PickCsv {
                kind: DialogRequestKind::AddFile,
                path: Some(path),
            } => match self.data_store.load_csv_file(path, false) {
                Ok(_) => {
                    self.fit_next_frame = true;
                    self.last_error = None;
                }
                Err(err) => {
                    self.last_error = Some(format!("Failed to add CSV: {err}"));
                }
            },
            DialogOutcome::SaveExport {
                selection,
                path: Some(save_path),
            } => match export_to_csv(&save_path, &self.data_store, selection) {
                Ok(()) => {
                    self.last_error = None;
                }
                Err(err) => {
                    self.last_error = Some(format!("Export failed: {err}"));
                }
            },
            DialogOutcome::PickCsv { path: None, .. }
            | DialogOutcome::SaveExport { path: None, .. } => {}
        }
    }

    pub fn handle_runtime_event(&mut self, event: &RuntimeEvent) {
        self.data_store.handle_runtime_event(event);
    }

    fn render_chart(&mut self, ui: &mut egui::Ui) {
        if self.data_store.file_count() == 0 {
            self.overlay.cursor_pos = None;
            self.clear_hover_state();
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
        let active_ids: Vec<_> = self.data_store.files().iter().map(|file| file.id).collect();
        self.render_cache.retain_sources(&active_ids);
        let mut consumed_follow_snap = false;
        let plot_response = egui_plot::Plot::new(GRAPH_VIEWER_PLOT_ID)
            .allow_zoom(true)
            .allow_drag(self.interaction_mode == InteractionMode::Normal)
            .allow_scroll(true)
            .x_axis_formatter(move |mark, _range| format_axis_label(mark.value, active_domain))
            .label_formatter(|_, _| String::new())
            .show(ui, |plot_ui| {
                let hovering_plot = plot_ui.response().hovered() || plot_ui.response().dragged();
                let cursor_pos = if hovering_plot {
                    plot_ui.pointer_coordinate()
                } else {
                    None
                };
                let pointer_screen_pos = if hovering_plot {
                    plot_ui
                        .response()
                        .interact_pointer_pos()
                        .or_else(|| plot_ui.response().hover_pos())
                } else {
                    None
                };
                self.overlay.cursor_pos = cursor_pos;
                self.refresh_hover_state(pointer_screen_pos, cursor_pos);
                let visible_bounds = plot_ui.plot_bounds().range_x();
                let plot_query = build_plot_query(
                    Some((*visible_bounds.start(), *visible_bounds.end())),
                    Some(plot_ui.response().rect.width()),
                );
                let current_span = plot_query
                    .x_range
                    .map(|(lo, hi)| (hi - lo).abs())
                    .unwrap_or(0.0);

                for file in self.data_store.files() {
                    if !file.visible {
                        continue;
                    }

                    if self
                        .render_cache
                        .get_series(
                            file.id,
                            file.render_revision,
                            plot_query.x_range,
                            current_span,
                            plot_query.target_points,
                        )
                        .is_some()
                    {
                        continue;
                    }

                    let source_id = file.id;
                    let revision = file.render_revision;
                    let points = self.data_store.query_points_in_view(
                        source_id,
                        plot_query.x_range,
                        plot_query.target_points,
                    );
                    let series: Vec<egui_plot::PlotPoint> = points
                        .into_iter()
                        .map(|(time, value)| egui_plot::PlotPoint::new(time.as_secs_f64(), value))
                        .collect();

                    self.render_cache.store_series(
                        source_id,
                        revision,
                        plot_query.x_range,
                        current_span,
                        plot_query.target_points,
                        series,
                    );
                }

                for file in self.data_store.files() {
                    if !file.visible {
                        continue;
                    }

                    if let Some(cached) = self.render_cache.get_series(
                        file.id,
                        file.render_revision,
                        plot_query.x_range,
                        current_span,
                        plot_query.target_points,
                    ) {
                        if cached.is_empty() {
                            continue;
                        }

                        plot_ui.line(
                            egui_plot::Line::new(file.label.clone(), cached)
                                .stroke(egui::Stroke::new(1.5, file.color)),
                        );
                    }
                }

                overlay::draw_selection(plot_ui, self.overlay.selection);
                overlay::draw_measurements(plot_ui, &self.overlay, cursor_pos);
                overlay::draw_markers(plot_ui, &self.overlay.markers, &mode_change_markers);
                overlay::draw_crosshair(
                    plot_ui,
                    cursor_pos,
                    self.hovered_cursor.as_ref(),
                    self.hovered_live_now.as_ref(),
                );

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

    fn cursor_info_from_record(record: data_store::HoveredRecord) -> info_bar::CursorInfo {
        info_bar::CursorInfo {
            series: record.series,
            value: record.value,
            unit: record.unit,
            timestamp: record.timestamp,
            mode: record.mode,
        }
    }

    fn clear_hover_state(&mut self) {
        self.hovered_cursor = None;
        self.hovered_live_now = None;
        self.hovered_source_id = None;
        self.last_hover_screen_pos = None;
    }

    fn refresh_hover_state(
        &mut self,
        pointer_screen_pos: Option<Pos2>,
        cursor_pos: Option<egui_plot::PlotPoint>,
    ) {
        let (Some(pointer_screen_pos), Some(cursor_pos)) = (pointer_screen_pos, cursor_pos) else {
            self.clear_hover_state();
            return;
        };

        let pointer_moved = self
            .last_hover_screen_pos
            .is_none_or(|previous| previous.distance_sq(pointer_screen_pos) > 1.0);

        if pointer_moved || self.hovered_cursor.is_none() {
            let record = self
                .data_store
                .nearest_visible_sample(cursor_pos.x, cursor_pos.y);
            self.hovered_source_id = record.as_ref().map(|record| record.source_id);
            self.hovered_cursor = record.map(Self::cursor_info_from_record);
            self.last_hover_screen_pos = Some(pointer_screen_pos);
        }

        self.hovered_live_now = self
            .hovered_source_id
            .and_then(|source_id| self.data_store.latest_visible_live_sample(source_id))
            .map(Self::cursor_info_from_record);

        if self.hovered_cursor.is_none() {
            self.hovered_live_now = None;
            self.hovered_source_id = None;
        }
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
            self.dispatch_action(ViewerAction::AddFile, config);
            return;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.dispatch_action(ViewerAction::OpenFile, config);
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

fn dialog_request_for_action(
    action: ViewerAction,
    selection: Option<(f64, f64)>,
) -> Option<DialogRequest> {
    match action {
        ViewerAction::OpenFile => Some(DialogRequest::PickCsv {
            kind: DialogRequestKind::OpenFile,
        }),
        ViewerAction::AddFile => Some(DialogRequest::PickCsv {
            kind: DialogRequestKind::AddFile,
        }),
        ViewerAction::Export => Some(DialogRequest::SaveExport { selection }),
        _ => None,
    }
}

fn spawn_dialog_request(
    request: DialogRequest,
    ctx: egui::Context,
) -> mpsc::Receiver<DialogOutcome> {
    let (result_tx, result_rx) = mpsc::channel();

    match request {
        DialogRequest::PickCsv { kind } => {
            let file_future = rfd::AsyncFileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file();
            // IMPORTANT: Do not revert to .expect("dialog runtime") — this
            // has regressed 4 times. Graceful fallback prevents panic on
            // resource exhaustion.
            std::thread::spawn(move || {
                let path = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()
                    .and_then(|rt| rt.block_on(file_future).map(|f| f.path().to_path_buf()));
                let _ = result_tx.send(DialogOutcome::PickCsv { kind, path });
                ctx.request_repaint();
            });
        }
        DialogRequest::SaveExport { selection } => {
            let file_future = rfd::AsyncFileDialog::new()
                .add_filter("CSV", &["csv"])
                .set_file_name("export.csv")
                .save_file();
            std::thread::spawn(move || {
                let path = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()
                    .and_then(|rt| rt.block_on(file_future).map(|f| f.path().to_path_buf()));
                let _ = result_tx.send(DialogOutcome::SaveExport { selection, path });
                ctx.request_repaint();
            });
        }
    }

    result_rx
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

fn build_plot_query(x_range: Option<(f64, f64)>, plot_width_points: Option<f32>) -> PlotQuery {
    let target_points = plot_width_points
        .filter(|width| *width > 0.0)
        .map(visible_point_budget)
        .unwrap_or(SAFE_PLOT_QUERY_BUDGET);

    PlotQuery {
        x_range: x_range.map(|(start, end)| (start.min(end), start.max(end))),
        target_points,
    }
}

fn should_detach_follow(ui: &egui::Ui, response: &egui::Response, allow_drag_detach: bool) -> bool {
    let scrolled = response.hovered()
        && ui.input(|input| {
            input.smooth_scroll_delta != egui::Vec2::ZERO
                || input.raw_scroll_delta != egui::Vec2::ZERO
        });

    (allow_drag_detach && response.dragged()) || scrolled
}

fn follow_live_edge(plot_ui: &mut egui_plot::PlotUi<'_>, latest_x: f64, force_snap: bool) -> bool {
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
        CsvDataStore, DialogOutcome, DialogRequest, DialogRequestKind, GRAPH_VIEWER_PLOT_ID,
        GRAPH_VIEWER_VIEWPORT_ID, GRAPH_VIEWER_WINDOW_TITLE, GraphViewerWindow, ViewerAction,
        export_to_csv, format_csv_value,
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

    #[test]
    fn graph_viewer_cursor_live_latches_cursor_snapshot_while_live_now_updates() {
        let mut viewer = GraphViewerWindow::new();
        let mm = viewer
            .data_store
            .attach_runtime_device(DeviceId::Multimeter)
            .expect("attach multimeter");
        let usbc = viewer
            .data_store
            .attach_runtime_device(DeviceId::UsbC)
            .expect("attach usbc");

        viewer
            .data_store
            .push_test_sample(mm, 100.0, Some(1.0), "V", "DCV");
        viewer
            .data_store
            .push_test_sample(usbc, 100.0, Some(9.0), "V", "DCV");

        viewer.refresh_hover_state(
            Some(egui::pos2(10.0, 10.0)),
            Some(egui_plot::PlotPoint::new(100.0, 8.8)),
        );

        assert_eq!(viewer.hovered_cursor.as_ref().unwrap().series, "USB-C Live");
        assert_eq!(viewer.hovered_cursor.as_ref().unwrap().value, 9.0);
        assert_eq!(viewer.hovered_live_now.as_ref().unwrap().value, 9.0);

        viewer
            .data_store
            .push_test_sample(usbc, 101.0, Some(10.0), "V", "DCV");

        viewer.refresh_hover_state(
            Some(egui::pos2(10.0, 10.0)),
            Some(egui_plot::PlotPoint::new(101.0, 0.0)),
        );

        assert_eq!(viewer.hovered_cursor.as_ref().unwrap().series, "USB-C Live");
        assert_eq!(viewer.hovered_cursor.as_ref().unwrap().value, 9.0);
        assert_eq!(viewer.hovered_live_now.as_ref().unwrap().value, 10.0);
    }

    #[test]
    fn graph_viewer_cursor_live_resolves_first_sample_without_mouse_motion() {
        let mut viewer = GraphViewerWindow::new();
        let mm = viewer
            .data_store
            .attach_runtime_device(DeviceId::Multimeter)
            .expect("attach multimeter");
        let pointer_screen_pos = egui::pos2(10.0, 10.0);
        let cursor_pos = egui_plot::PlotPoint::new(100.0, 1.0);

        viewer.refresh_hover_state(Some(pointer_screen_pos), Some(cursor_pos));

        assert!(viewer.hovered_cursor.is_none());
        assert!(viewer.hovered_live_now.is_none());

        viewer
            .data_store
            .push_test_sample(mm, 100.0, Some(1.0), "V", "DCV");

        viewer.refresh_hover_state(Some(pointer_screen_pos), Some(cursor_pos));

        assert_eq!(viewer.hovered_cursor.as_ref().unwrap().series, "MM Live");
        assert_eq!(viewer.hovered_cursor.as_ref().unwrap().value, 1.0);
        assert_eq!(viewer.hovered_live_now.as_ref().unwrap().value, 1.0);
    }

    #[test]
    fn plot_query_uses_visible_bounds_when_available() {
        let query = super::build_plot_query(Some((10.0, 20.0)), Some(500.0));

        assert_eq!(query.x_range, Some((10.0, 20.0)));
        assert_eq!(query.target_points, 1000);
    }

    #[test]
    fn plot_query_falls_back_to_safe_budget_when_width_is_missing() {
        let query = super::build_plot_query(Some((0.0, 5.0)), None);

        assert_eq!(query.x_range, Some((0.0, 5.0)));
        assert_eq!(query.target_points, 256);
    }

    #[test]
    fn live_poll_interval_targets_50ms_updates() {
        assert_eq!(super::LIVE_POLL_INTERVAL, Duration::from_millis(50));
    }

    #[test]
    fn add_file_action_maps_to_async_csv_dialog_request() {
        let request = super::dialog_request_for_action(ViewerAction::AddFile, None);

        assert_eq!(
            request,
            Some(DialogRequest::PickCsv {
                kind: DialogRequestKind::AddFile,
            })
        );
    }

    #[test]
    fn queued_dialog_request_is_exposed_for_app_level_dispatch() {
        let mut viewer = GraphViewerWindow::new();
        let request = DialogRequest::PickCsv {
            kind: DialogRequestKind::AddFile,
        };

        viewer.queue_dialog_request(request.clone());

        assert!(viewer.pending_dialog.is_none());
        assert_eq!(viewer.take_dialog_request(), Some(request));
        assert_eq!(viewer.take_dialog_request(), None);
    }

    #[test]
    fn applying_add_file_dialog_outcome_loads_csv_without_clearing_existing_sources() {
        let existing_path = std::env::temp_dir().join(format!(
            "readout_graph_viewer_existing_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));
        let added_path = std::env::temp_dir().join(format!(
            "readout_graph_viewer_added_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));

        fs::write(
            &existing_path,
            concat!(
                "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
                "2026-03-29T10:00:00Z,Multimeter,1.25,V,DCV,false,false,false\n"
            ),
        )
        .expect("write existing csv");
        fs::write(
            &added_path,
            concat!(
                "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
                "2026-03-29T10:00:01Z,USB-C,5.00,V,DCV,false,false,false\n"
            ),
        )
        .expect("write added csv");

        let config = AppConfiguration::default();
        let mut viewer = GraphViewerWindow::new();
        viewer
            .data_store
            .load_csv_file(existing_path.clone(), true)
            .expect("load existing csv");

        let outcome = DialogOutcome::PickCsv {
            kind: DialogRequestKind::AddFile,
            path: Some(added_path.clone()),
        };
        viewer.apply_dialog_outcome(outcome, &config);

        assert_eq!(viewer.data_store.file_count(), 2);
        assert!(viewer.last_error.is_none());

        fs::remove_file(&existing_path).expect("remove existing csv");
        fs::remove_file(&added_path).expect("remove added csv");
    }
}
