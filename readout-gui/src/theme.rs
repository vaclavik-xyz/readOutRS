use readout_persistence::config::DashboardTheme;

pub fn apply_theme(ctx: &egui::Context, theme: DashboardTheme) {
    let mut visuals = match theme {
        DashboardTheme::Light => light_visuals(),
        DashboardTheme::Dark | DashboardTheme::System => dark_visuals(),
    };

    // Shared tweaks
    let corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = corner_radius;
    visuals.widgets.inactive.corner_radius = corner_radius;
    visuals.widgets.hovered.corner_radius = corner_radius;
    visuals.widgets.active.corner_radius = corner_radius;

    ctx.set_visuals(visuals);
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();

    // Background colors — dark blue-gray
    v.panel_fill = egui::Color32::from_rgb(14, 17, 23);
    v.window_fill = egui::Color32::from_rgb(20, 24, 32);
    v.extreme_bg_color = egui::Color32::from_rgb(8, 10, 14);
    v.faint_bg_color = egui::Color32::from_rgb(24, 28, 38);

    // Widget backgrounds
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(24, 28, 38);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(200));
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(25));

    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 36, 48);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(180));
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(20));

    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 48, 64);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40));

    v.widgets.active.bg_fill = egui::Color32::from_rgb(50, 60, 80);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

    // Selection
    v.selection.bg_fill = egui::Color32::from_rgb(40, 80, 140);
    v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 160, 255));

    // Window stroke
    v.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(20));

    // Separator
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(18));

    v
}

fn light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();

    // Background — light blue-gray
    v.panel_fill = egui::Color32::from_rgb(230, 238, 248);
    v.window_fill = egui::Color32::from_rgb(240, 244, 250);
    v.extreme_bg_color = egui::Color32::from_rgb(250, 252, 255);
    v.faint_bg_color = egui::Color32::from_rgb(220, 228, 240);

    // Text
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(23, 28, 36));
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 46, 56));

    // Selection
    v.selection.bg_fill = egui::Color32::from_rgb(180, 215, 255);

    v
}
