mod data_store;
mod info_bar;
mod viewer_toolbar;

use self::data_store::CsvDataStore;
use readout_persistence::config::AppConfiguration;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct CsvViewerWindow {
    pub open: bool,
    data_store: CsvDataStore,
    interaction_mode: InteractionMode,
    following: bool,
    last_poll: Instant,
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
}

impl CsvViewerWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            data_store: CsvDataStore::new(),
            interaction_mode: InteractionMode::Normal,
            following: true,
            last_poll: Instant::now(),
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

                if has_live_files && self.last_poll.elapsed() >= Duration::from_secs(1) {
                    self.data_store.poll_live_files();
                    self.last_poll = Instant::now();
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    let action = viewer_toolbar::show(
                        ui,
                        &self.data_store,
                        self.interaction_mode,
                        self.following,
                    );
                    self.handle_action(action, &live_paths);

                    ui.separator();
                    self.render_chart(ui);
                    ui.separator();
                    info_bar::show(ui, None, None, None);
                });

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
                    self.data_store = CsvDataStore::new();
                    let _ = self.data_store.load_file(path, is_live);
                    self.following = is_live;
                }
            }
            ViewerAction::AddFile => {
                if let Some(path) = pick_csv_file() {
                    let is_live = is_live_path(&path, live_paths);
                    let _ = self.data_store.load_file(path, is_live);
                    if is_live {
                        self.following = true;
                    }
                }
            }
            ViewerAction::SetMode(mode) => {
                self.interaction_mode = mode;
            }
            ViewerAction::ToggleFollow => {
                self.following = !self.following;
            }
            ViewerAction::ZoomFit | ViewerAction::Export | ViewerAction::None => {}
        }
    }

    fn render_chart(&mut self, ui: &mut egui::Ui) {
        if self.data_store.file_count() == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Open a CSV file to inspect logged measurements.")
                        .weak(),
                );
            });
            return;
        }

        egui_plot::Plot::new("csv_viewer_plot")
            .legend(egui_plot::Legend::default())
            .allow_zoom(true)
            .allow_drag(true)
            .allow_scroll(true)
            .show(ui, |plot_ui| {
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
                        egui_plot::Line::new(name, series).stroke(egui::Stroke::new(1.5, file.color)),
                    );
                }
            });
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

#[cfg(test)]
mod tests {
    use super::{CsvViewerWindow, ViewerAction};
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
}
