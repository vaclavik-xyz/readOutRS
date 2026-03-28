# Check for Updates — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional startup check for new GitHub releases, shown in Settings panels of both GUI and TUI.

**Architecture:** `ureq` HTTP client calls GitHub releases API in `spawn_blocking` at startup. Result stored in `DashboardState.update_available: Option<String>`. Both frontends display it in an "About" section at the bottom of their Settings panel.

**Tech Stack:** ureq, serde_json (existing), semver comparison via string parsing

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` (workspace) | Modify | Add ureq to workspace deps |
| `crates/readout-core/Cargo.toml` | Modify | Add ureq dependency |
| `crates/readout-core/src/update_checker.rs` | Create | GitHub release check + version compare |
| `crates/readout-core/src/lib.rs` | Modify | Export update_checker module |
| `crates/readout-core/src/dashboard_state.rs` | Modify | Add update_available field |
| `crates/readout-persistence/src/config.rs` | Modify | Add check_for_updates bool |
| `readout-gui/src/app.rs` | Modify | Trigger check on startup |
| `readout-gui/src/widgets/settings.rs` | Modify | About section with version + update |
| `readout-tui/src/app.rs` | Modify | Trigger check on startup |
| `readout-tui/src/widgets/settings.rs` | Modify | About section with version + update |

---

## Task 1: Add ureq dependency and update checker module

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/readout-core/Cargo.toml`
- Create: `crates/readout-core/src/update_checker.rs`
- Modify: `crates/readout-core/src/lib.rs`

- [ ] **Step 1: Add ureq to workspace and readout-core**

- [ ] **Step 2: Create update_checker.rs**

```rust
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/vaclavik-xyz/readOutRS/releases/latest";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn check_for_update() -> Option<String> {
    let response = ureq::get(GITHUB_RELEASES_URL)
        .header("User-Agent", "readOutRS")
        .call()
        .ok()?;
    let body: serde_json::Value = response.body().read_json().ok()?;
    let tag = body["tag_name"].as_str()?;
    let remote = tag.strip_prefix('v').unwrap_or(tag);
    if is_newer(remote, CURRENT_VERSION) {
        Some(remote.to_string())
    } else {
        None
    }
}

fn is_newer(remote: &str, current: &str) -> bool {
    // Simple semver comparison: split by dots, compare numerically
    let parse = |s: &str| -> Vec<u32> {
        s.split('.').filter_map(|p| p.parse().ok()).collect()
    };
    let r = parse(remote);
    let c = parse(current);
    r > c
}
```

- [ ] **Step 3: Export module in lib.rs**

- [ ] **Step 4: Build and verify**

Run: `cargo check -p readout-core`

- [ ] **Step 5: Commit**

---

## Task 2: Add config field and state field

**Files:**
- Modify: `crates/readout-persistence/src/config.rs`
- Modify: `crates/readout-core/src/dashboard_state.rs`

- [ ] **Step 1: Add `check_for_updates: bool` to AppConfiguration (default true)**

- [ ] **Step 2: Add `update_available: Option<String>` to DashboardState**

- [ ] **Step 3: Build and verify**

- [ ] **Step 4: Commit**

---

## Task 3: Wire startup check in GUI and TUI

**Files:**
- Modify: `readout-gui/src/app.rs`
- Modify: `readout-tui/src/app.rs`

- [ ] **Step 1: GUI — spawn_blocking check after runtime starts**

```rust
if config.check_for_updates {
    let tx = update_tx.clone(); // or store in Arc
    tokio::task::spawn_blocking(move || {
        if let Some(version) = readout_core::update_checker::check_for_update() {
            let _ = tx.send(version);
        }
    });
}
```

- [ ] **Step 2: TUI — same pattern in run()**

- [ ] **Step 3: Build and verify**

- [ ] **Step 4: Commit**

---

## Task 4: GUI Settings About section

**Files:**
- Modify: `readout-gui/src/widgets/settings.rs`

- [ ] **Step 1: Add About section at bottom of settings panel**

Show current version, update status, and "Open" button linking to releases page.

- [ ] **Step 2: Add check_for_updates toggle**

- [ ] **Step 3: Build and verify**

- [ ] **Step 4: Commit**

---

## Task 5: TUI Settings About section

**Files:**
- Modify: `readout-tui/src/widgets/settings.rs`

- [ ] **Step 1: Add About separator and version/update fields**

- [ ] **Step 2: Add check_for_updates toggle**

- [ ] **Step 3: Build and verify**

- [ ] **Step 4: Commit**

---

## Task 6: Full build and test

- [ ] **Step 1: cargo build**
- [ ] **Step 2: cargo test**
- [ ] **Step 3: Final commit if needed**
