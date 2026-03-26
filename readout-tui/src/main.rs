mod app;

use clap::Parser;
use readout_persistence::config_store;

#[derive(Parser)]
#[command(name = "readout-tui", about = "readout terminal measurement dashboard")]
struct Cli {
    /// Path to config file
    #[arg(long)]
    config: Option<std::path::PathBuf>,

    /// Force simulator mode
    #[arg(long)]
    simulator: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let config_path = cli
        .config
        .unwrap_or_else(config_store::default_config_path);

    let mut config = config_store::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config: {e:?}, using defaults");
        readout_persistence::config::AppConfiguration::default()
    });

    if cli.simulator {
        config.use_simulator = true;
    }

    if let Err(e) = app::run(config).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
