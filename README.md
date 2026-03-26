# readOutRS

Real-time measurement dashboard for multimeters (SCPI) and USB-C power meters.

Rust rewrite of [readOut](https://github.com/vaclavik-xyz/readOut) with two frontends:
- **readout-gui** — desktop app (egui)
- **readout-tui** — terminal dashboard (ratatui)

Cross-platform: macOS, Linux, Windows.

## Build

```bash
cargo build
```

## Run

```bash
cargo run -p readout-gui
cargo run -p readout-tui
```

## Test

```bash
cargo test
```
