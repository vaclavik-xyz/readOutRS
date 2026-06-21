mod app;
mod audio;
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

fn load_icon() -> egui::IconData {
    let png_bytes = include_bytes!("../assets/icon_256x256.png");
    let image = image::load_from_memory(png_bytes).expect("Failed to load icon");
    let rgba = image.to_rgba8();
    egui::IconData {
        rgba: rgba.to_vec(),
        width: rgba.width(),
        height: rgba.height(),
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let explicit_config_path = cli.config.is_some();
    let config_path = cli.config.unwrap_or_else(config_store::default_config_path);

    let startup_config = config_store::load_for_startup(&config_path, explicit_config_path)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to load config {}: {:?}", config_path.display(), e);
            eprintln!("Failed to load config {}: {e:?}", config_path.display());
            std::process::exit(1);
        });
    let first_run = startup_config.first_run;
    let mut config = startup_config.config;

    if cli.simulator {
        config.use_simulator = true;
    }

    let icon = load_icon();
    let always_on_top = config.always_on_top;
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(app::initial_window_size())
        .with_resizable(true)
        .with_titlebar_shown(true)
        .with_title_shown(true)
        .with_fullsize_content_view(false)
        .with_movable_by_background(false)
        .with_icon(icon);
    if always_on_top {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "readOut",
        options,
        Box::new(move |cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Bold);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app::ReadOutApp::new(
                config,
                config_path,
                first_run,
                &cc.egui_ctx,
            )))
        }),
    )
    .expect("eframe run");
}
