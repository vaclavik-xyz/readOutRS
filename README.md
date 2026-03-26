# readOutRS

Real-time measurement dashboard for multimeters (SCPI) and USB-C power meters.

Rust rewrite of [readOut](https://github.com/vaclavik-xyz/readOut) with two frontends:
- **readout-gui** — desktop app (egui)
- **readout-tui** — terminal dashboard (ratatui)

Cross-platform: macOS, Linux, Windows.

## Architecture

```
readOutRS/
├── crates/
│   ├── readout-core/         # data types, parsers, alerts, chart pipeline
│   ├── readout-io/           # serial port, transports, device sessions, runtime
│   └── readout-persistence/  # config, CSV logger, OBS writer
├── readout-gui/              # egui desktop application
└── readout-tui/              # ratatui terminal dashboard
```

Channel-based event bus (`tokio::sync::broadcast`) connects backend to frontends. Each device runs as an independent Tokio task.

## Build

```bash
cargo build
```

## Run

```bash
# Desktop GUI
cargo run -p readout-gui

# Terminal TUI
cargo run -p readout-tui

# With simulator (no hardware needed)
cargo run -p readout-gui -- --simulator
cargo run -p readout-tui -- --simulator
```

## Test

```bash
# Unit + integration tests
cargo test

# Soak tests (longer running, behind feature flag)
cargo test -p readout-io --features soak -- soak --nocapture
```

## Keyboard shortcuts

### GUI
- `Cmd+P` / `Ctrl+P` — pause/resume
- `Cmd+L` / `Ctrl+L` — toggle log panel
- `Cmd+,` / `Ctrl+,` — settings
- `Cmd+1` / `Ctrl+1` — multimeter popout
- `Cmd+2` / `Ctrl+2` — USB-C popout

### TUI
- `q` — quit
- `p` — pause/resume
- `s` — settings
- `←` / `→` — chart range
