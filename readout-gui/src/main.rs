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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MainViewportChrome {
    titlebar_shown: bool,
    title_shown: bool,
    fullsize_content_view: bool,
    movable_by_background: bool,
}

fn main_viewport_chrome() -> MainViewportChrome {
    MainViewportChrome {
        titlebar_shown: true,
        title_shown: true,
        fullsize_content_view: false,
        movable_by_background: false,
    }
}

fn build_main_viewport(icon: egui::IconData, always_on_top: bool) -> egui::ViewportBuilder {
    let chrome = main_viewport_chrome();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(app::initial_window_size())
        .with_resizable(true)
        .with_titlebar_shown(chrome.titlebar_shown)
        .with_title_shown(chrome.title_shown)
        .with_fullsize_content_view(chrome.fullsize_content_view)
        .with_movable_by_background(chrome.movable_by_background)
        .with_icon(icon);
    if always_on_top {
        viewport = viewport.with_always_on_top();
    }
    viewport
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let config_path = cli.config.unwrap_or_else(config_store::default_config_path);

    let first_run = !config_path.exists();

    let mut config = config_store::load(&config_path).unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {:?}, using defaults", e);
        readout_persistence::config::AppConfiguration::default()
    });

    if cli.simulator {
        config.use_simulator = true;
    }

    let icon = load_icon();
    let always_on_top = config.always_on_top;
    let options = eframe::NativeOptions {
        viewport: build_main_viewport(icon, always_on_top),
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

#[cfg(test)]
mod tests {
    use super::{MainViewportChrome, main_viewport_chrome};

    #[test]
    fn native_title_bar_main_window_chrome_uses_system_title_bar() {
        assert_eq!(
            main_viewport_chrome(),
            MainViewportChrome {
                titlebar_shown: true,
                title_shown: true,
                fullsize_content_view: false,
                movable_by_background: false,
            }
        );
    }
}
