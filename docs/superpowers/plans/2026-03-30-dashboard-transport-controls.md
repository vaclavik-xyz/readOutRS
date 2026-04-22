# Dashboard Transport Controls Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit `Play/Pause` and `Stop` transport controls to the dashboard title bar and context menu, with `Stop` performing `clear + pause` while CSV logging keeps running unchanged.

**Architecture:** Reuse the existing dashboard `TogglePause` and `ClearCharts` actions instead of introducing new runtime/logging controls. Keep the internal action name `ClearCharts`, move the dashboard toolbar action handling behind a small `ReadOutApp::apply_toolbar_action(...)` method for testability, and drive the new UI text/icon behavior from toolbar-local helper functions so title bar and context menu stay consistent.

**Tech Stack:** Rust, `egui`, `egui_phosphor`, `readout_core::dashboard_state::DashboardState`, `readout_core::chart_pipeline::ChartPipeline`

**Spec:** `docs/superpowers/specs/2026-03-30-dashboard-transport-controls-design.md`

## Preflight

- This repository is currently on `main`. Do not execute implementation directly on `main`.
- Before Task 1, use `@using-git-worktrees` to create a dedicated worktree/branch for this feature.
- Do not commit unrelated untracked files already present in the repository.

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `readout-gui/src/widgets/toolbar.rs` | Transport copy/icon helpers, `TitleBarState.paused`, title bar/context menu rendering |
| Modify | `readout-gui/src/app.rs` | Pass paused state into toolbar, apply `Stop = clear + pause`, keep CSV logging semantics unchanged, add app regression tests |

---

## Chunk 1: Toolbar Transport Surface

### Task 1: Add transport copy/icon helpers in `toolbar.rs`

**Files:**
- Modify: `readout-gui/src/widgets/toolbar.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `toolbar.rs`:

```rust
#[test]
fn dashboard_transport_pause_copy_switches_with_state() {
    assert_eq!(
        pause_transport_label(false),
        format!("{} Pause", icons::PAUSE)
    );
    assert_eq!(
        pause_transport_label(true),
        format!("{} Play", icons::PLAY)
    );
    assert_eq!(pause_transport_tooltip(false), "Pause dashboard updates");
    assert_eq!(pause_transport_tooltip(true), "Resume dashboard updates");
}

#[test]
fn dashboard_transport_stop_copy_uses_stop_icon_and_explicit_tooltip() {
    assert_eq!(stop_transport_label(), format!("{} Stop", icons::STOP));
    assert_eq!(
        stop_transport_tooltip(),
        "Stop dashboard charts (clear and pause)"
    );
    assert_ne!(stop_transport_label(), format!("{} Clear Charts", icons::TRASH));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-gui dashboard_transport_`
Expected: FAIL because the helper functions do not exist yet.

- [ ] **Step 3: Implement the minimal helpers**

In `toolbar.rs`, add helper functions near the toolbar action definitions:

```rust
fn pause_transport_label(paused: bool) -> String {
    if paused {
        format!("{} Play", icons::PLAY)
    } else {
        format!("{} Pause", icons::PAUSE)
    }
}

fn pause_transport_tooltip(paused: bool) -> &'static str {
    if paused {
        "Resume dashboard updates"
    } else {
        "Pause dashboard updates"
    }
}

fn stop_transport_label() -> String {
    format!("{} Stop", icons::STOP)
}

fn stop_transport_tooltip() -> &'static str {
    "Stop dashboard charts (clear and pause)"
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p readout-gui dashboard_transport_`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/widgets/toolbar.rs
git commit -m "feat(gui): add dashboard transport copy helpers"
```

---

### Task 2: Wire `Play/Pause` and `Stop` into the title bar and context menu

**Files:**
- Modify: `readout-gui/src/widgets/toolbar.rs`

- [ ] **Step 1: Write the failing test**

Add to `toolbar.rs` tests:

```rust
#[test]
fn title_bar_state_carries_paused_transport_state() {
    let state = TitleBarState {
        always_on_top: false,
        selected_range_idx: 1,
        show_mm: true,
        show_usbc: true,
        paused: true,
    };

    assert!(state.paused);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p readout-gui title_bar_state_carries_paused_transport_state`
Expected: Compilation error because `TitleBarState` does not have a `paused` field yet.

- [ ] **Step 3: Implement the minimal UI wiring**

In `toolbar.rs`:

- add `pub paused: bool` to `TitleBarState`
- in `show_title_bar(...)`, add a `Play/Pause` button next to the existing chart-clear slot
- use `pause_transport_label(state.paused)` and `pause_transport_tooltip(state.paused)`
- keep the action as `ToolbarAction::TogglePause`
- replace the trash-title-bar button label/tooltip with `stop_transport_label()` / `stop_transport_tooltip()`
- keep the action as `ToolbarAction::ClearCharts`
- in `context_menu(...)`, reuse the same helper functions for the `TogglePause` and `ClearCharts` entries

- [ ] **Step 4: Run targeted tests**

Run: `cargo test -p readout-gui title_bar_state_carries_paused_transport_state`
Expected: PASS

- [ ] **Step 5: Run the toolbar test group**

Run: `cargo test -p readout-gui dashboard_transport_`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/widgets/toolbar.rs
git commit -m "feat(gui): add dashboard play pause and stop controls"
```

---

## Chunk 2: App-Level Semantics

### Task 3: Extract toolbar action handling into a testable app method

**Files:**
- Modify: `readout-gui/src/app.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `app.rs`:

```rust
fn test_app(ctx: &egui::Context) -> ReadOutApp {
    ReadOutApp::new(
        AppConfiguration::default(),
        std::path::PathBuf::from("config.toml"),
        true,
        ctx,
    )
}

#[test]
fn toolbar_action_toggle_pause_only_flips_paused_state() {
    let ctx = egui::Context::default();
    let mut app = test_app(&ctx);
    app.state.paused = false;

    app.apply_toolbar_action(crate::widgets::toolbar::ToolbarAction::TogglePause, &ctx);

    assert!(app.state.paused);
}

#[test]
fn toolbar_action_clear_charts_clears_all_pipelines_and_pauses_dashboard() {
    let ctx = egui::Context::default();
    let mut app = test_app(&ctx);
    app.state.paused = false;

    app.state
        .chart_pipelines
        .get_mut(&DeviceId::Multimeter)
        .unwrap()
        .push(Duration::from_secs(1), 1.0);
    app.state
        .usbc_chart_pipelines
        .get_mut(&UsbCMetric::Voltage)
        .unwrap()
        .push(Duration::from_secs(1), 5.0);

    app.apply_toolbar_action(crate::widgets::toolbar::ToolbarAction::ClearCharts, &ctx);

    assert!(app.state.paused);
    assert_eq!(
        app.state
            .chart_pipelines
            .get_mut(&DeviceId::Multimeter)
            .unwrap()
            .query(Duration::from_secs(10), 128),
        Vec::new()
    );
    assert_eq!(
        app.state
            .usbc_chart_pipelines
            .get_mut(&UsbCMetric::Voltage)
            .unwrap()
            .query(Duration::from_secs(10), 128),
        Vec::new()
    );
}

#[test]
fn toolbar_action_clear_charts_leaves_csv_logging_flags_unchanged() {
    let ctx = egui::Context::default();
    let mut app = test_app(&ctx);
    app.config.multimeter_csv_logging_enabled = true;
    app.config.usbc_csv_logging_enabled = true;

    app.apply_toolbar_action(crate::widgets::toolbar::ToolbarAction::ClearCharts, &ctx);

    assert!(app.config.multimeter_csv_logging_enabled);
    assert!(app.config.usbc_csv_logging_enabled);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p readout-gui toolbar_action_`
Expected: FAIL because `apply_toolbar_action(...)` does not exist yet.

- [ ] **Step 3: Implement the minimal extraction**

In `app.rs`, add a method on `ReadOutApp`:

```rust
fn apply_toolbar_action(
    &mut self,
    toolbar_action: widgets::toolbar::ToolbarAction,
    ctx: &egui::Context,
) {
    match toolbar_action {
        widgets::toolbar::ToolbarAction::TogglePause => {
            self.state.paused = !self.state.paused;
        }
        widgets::toolbar::ToolbarAction::ClearCharts => {
            for pipeline in self.state.chart_pipelines.values_mut() {
                pipeline.clear();
            }
            for pipeline in self.state.usbc_chart_pipelines.values_mut() {
                pipeline.clear();
            }
            self.state.paused = true;
        }
        // move the remaining toolbar action arms here unchanged
        _ => { /* existing match arms moved from update() */ }
    }
}
```

Keep the existing behavior of all other toolbar actions unchanged.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p readout-gui toolbar_action_`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add readout-gui/src/app.rs
git commit -m "feat(gui): make dashboard stop action clear and pause"
```

---

### Task 4: Pass paused state into the title bar and route update through the new method

**Files:**
- Modify: `readout-gui/src/app.rs`

- [ ] **Step 1: Write the failing test**

Add to `app.rs` tests:

```rust
#[test]
fn build_title_bar_state_threads_dashboard_paused_flag() {
    let ctx = egui::Context::default();
    let mut app = test_app(&ctx);
    app.state.paused = true;

    let title_state = app.build_title_bar_state();

    assert!(title_state.paused);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p readout-gui build_title_bar_state_threads_dashboard_paused_flag`
Expected: FAIL because `build_title_bar_state()` does not exist yet.

- [ ] **Step 3: Implement the minimal routing change**

In `app.rs`:

- add a small helper:

```rust
fn build_title_bar_state(&self) -> widgets::toolbar::TitleBarState {
    widgets::toolbar::TitleBarState {
        always_on_top: self.always_on_top,
        selected_range_idx: self.selected_range_idx,
        show_mm: self.show_mm,
        show_usbc: self.show_usbc,
        paused: self.state.paused,
    }
}
```

- replace the inline `TitleBarState { ... }` construction in `update(...)` with:

```rust
let title_state = self.build_title_bar_state();
```

- replace the inline `match toolbar_action { ... }` block in `update(...)` with:

```rust
self.apply_toolbar_action(toolbar_action, ctx);
```

Keep the device section action handling unchanged.

- [ ] **Step 4: Run the targeted tests**

Run: `cargo test -p readout-gui build_title_bar_state_threads_dashboard_paused_flag`
Expected: PASS

- [ ] **Step 5: Run the app regression group**

Run: `cargo test -p readout-gui toolbar_action_`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add readout-gui/src/app.rs
git commit -m "refactor(gui): route dashboard toolbar actions through app helper"
```

---

## Chunk 3: Verification

### Task 5: Run final verification

**Files:**
- Verify only: `readout-gui/src/widgets/toolbar.rs`, `readout-gui/src/app.rs`

- [ ] **Step 1: Run the readout-gui test suite**

Run: `cargo test -p readout-gui`
Expected: PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p readout-gui -- -D warnings`
Expected: PASS with no warnings

- [ ] **Step 3: Run a build**

Run: `cargo build -p readout-gui`
Expected: PASS

- [ ] **Step 4: Manual smoke test**

Verify manually:

- title bar shows `Play/Pause` plus `Stop`
- `Pause` freezes dashboard values and charts
- `Stop` clears chart history and leaves dashboard paused
- resuming with `Play` fills charts only with new samples
- toggling device-level CSV logging still works independently

- [ ] **Step 5: Final commit if verification requires cleanup**

```bash
git add readout-gui/src/widgets/toolbar.rs readout-gui/src/app.rs
git commit -m "chore(gui): clean up dashboard transport controls verification fixes"
```
