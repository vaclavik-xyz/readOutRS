use readout_persistence::config::DashboardTheme;

/// Shared accent and semantic colors used across all widgets.
pub mod colors {
    use egui::Color32;

    pub const ACCENT: Color32 = Color32::from_rgb(0, 200, 175);

    pub const MM_LINE: Color32 = Color32::from_rgb(60, 170, 250);
    pub const USBC_LINE: Color32 = Color32::from_rgb(255, 160, 60);

    pub const CONNECTED: Color32 = Color32::from_rgb(45, 210, 120);
    pub const CONNECTING: Color32 = Color32::from_rgb(255, 195, 40);
    pub const DISCONNECTED: Color32 = Color32::from_rgb(100, 110, 130);
    pub const ERROR: Color32 = Color32::from_rgb(245, 65, 70);

    pub const ALARM_RED: Color32 = Color32::from_rgb(245, 60, 60);
    pub const ALARM_ORANGE: Color32 = Color32::from_rgb(235, 145, 20);
    pub const ALARM_YELLOW: Color32 = Color32::from_rgb(225, 205, 40);
}

/// Secondary text color for the active theme.
pub fn text_secondary(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(120, 130, 150)
    } else {
        egui::Color32::from_rgb(95, 105, 125)
    }
}

/// Semi-transparent variant of a color.
pub fn with_alpha(c: egui::Color32, alpha: u8) -> egui::Color32 {
    let [r, g, b, _] = c.to_array();
    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

/// Blend a tint into a base color at the given ratio (0.0–1.0).
pub fn tint(base: egui::Color32, tr: u8, tg: u8, tb: u8, amount: f32) -> egui::Color32 {
    let [br, bg, bb, _] = base.to_array();
    let mix = |b: u8, t: u8| ((b as f32) * (1.0 - amount) + (t as f32) * amount) as u8;
    egui::Color32::from_rgb(mix(br, tr), mix(bg, tg), mix(bb, tb))
}

pub fn apply_theme(ctx: &egui::Context, theme: DashboardTheme) {
    let visuals = match theme {
        DashboardTheme::Light => light_visuals(),
        DashboardTheme::Dark => dark_visuals(),
        DashboardTheme::System => {
            if is_system_dark_mode() {
                dark_visuals()
            } else {
                light_visuals()
            }
        }
    };
    ctx.set_visuals(visuals);
}

fn is_system_dark_mode() -> bool {
    use std::sync::Mutex;
    use std::time::Instant;

    static CACHE: Mutex<Option<(bool, Instant)>> = Mutex::new(None);
    const TTL: std::time::Duration = std::time::Duration::from_secs(5);

    if let Ok(guard) = CACHE.lock()
        && let Some((value, ts)) = *guard
        && ts.elapsed() < TTL
    {
        return value;
    }

    let result = query_system_dark_mode();

    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((result, Instant::now()));
    }

    result
}

fn query_system_dark_mode() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().eq_ignore_ascii_case("dark"))
            .unwrap_or(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    let r = egui::CornerRadius::same(4);

    // Surfaces — deep dark blue-gray
    v.panel_fill = egui::Color32::from_rgb(10, 12, 16);
    v.window_fill = egui::Color32::from_rgb(16, 19, 26);
    v.extreme_bg_color = egui::Color32::from_rgb(6, 8, 11);
    v.faint_bg_color = egui::Color32::from_rgb(22, 26, 35);

    let border = egui::Color32::from_rgb(35, 42, 55);

    // Non-interactive (labels, frames)
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(16, 19, 26);
    v.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(215, 220, 230));
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border);
    v.widgets.noninteractive.corner_radius = r;

    // Inactive (buttons at rest)
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(22, 26, 35);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 185, 200));
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);
    v.widgets.inactive.corner_radius = r;

    // Hovered
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(28, 35, 48);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 140, 125));
    v.widgets.hovered.corner_radius = r;

    // Active (pressed / selected)
    v.widgets.active.bg_fill = egui::Color32::from_rgb(15, 45, 50);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, colors::ACCENT);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, colors::ACCENT);
    v.widgets.active.corner_radius = r;

    // Open (combo boxes, menus)
    v.widgets.open.bg_fill = egui::Color32::from_rgb(22, 28, 40);
    v.widgets.open.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.widgets.open.corner_radius = r;

    // Selection
    v.selection.bg_fill = egui::Color32::from_rgb(0, 70, 62);
    v.selection.stroke = egui::Stroke::new(1.0, colors::ACCENT);

    // Window chrome
    v.window_stroke = egui::Stroke::new(1.0, border);
    v.window_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: egui::Color32::from_black_alpha(80),
    };

    v
}

fn light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    let r = egui::CornerRadius::same(4);

    let border = egui::Color32::from_rgb(210, 215, 228);

    // Surfaces — cool light gray
    v.panel_fill = egui::Color32::from_rgb(240, 242, 247);
    v.window_fill = egui::Color32::WHITE;
    v.extreme_bg_color = egui::Color32::from_rgb(248, 250, 254);
    v.faint_bg_color = egui::Color32::from_rgb(232, 236, 244);

    // Non-interactive
    v.widgets.noninteractive.bg_fill = egui::Color32::WHITE;
    v.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(18, 22, 32));
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border);
    v.widgets.noninteractive.corner_radius = r;

    // Inactive
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(232, 236, 245);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 56, 72));
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);
    v.widgets.inactive.corner_radius = r;

    // Hovered
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 228, 242);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(18, 22, 32));
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 140, 125));
    v.widgets.hovered.corner_radius = r;

    // Active
    v.widgets.active.bg_fill = egui::Color32::from_rgb(210, 240, 235);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 120, 105));
    v.widgets.active.corner_radius = r;

    // Selection
    v.selection.bg_fill = egui::Color32::from_rgb(190, 235, 228);
    v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 132));

    // Window chrome
    v.window_stroke = egui::Stroke::new(1.0, border);
    v.window_shadow = egui::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: egui::Color32::from_black_alpha(18),
    };

    v
}
