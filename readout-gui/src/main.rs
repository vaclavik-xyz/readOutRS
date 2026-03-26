mod app;
mod audio;
mod popout;
mod theme;
mod widgets;

use clap::Parser;
use readout_persistence::config_store;

#[derive(Parser)]
#[command(name = "readout-gui", about = "readout desktop measurement dashboard")]
struct Cli {
    /// Path to config file (overrides default location)
    #[arg(long)]
    config: Option<std::path::PathBuf>,

    /// Force simulator mode
    #[arg(long)]
    simulator: bool,
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let config_path = cli
        .config
        .unwrap_or_else(config_store::default_config_path);

    let first_run = !config_path.exists();

    let mut config = config_store::load(&config_path).unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {:?}, using defaults", e);
        readout_persistence::config::AppConfiguration::default()
    });

    if cli.simulator {
        config.use_simulator = true;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "readout",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::ReadOutApp::new(config, config_path, first_run, &cc.egui_ctx)))
        }),
    )
    .expect("eframe run");
}
