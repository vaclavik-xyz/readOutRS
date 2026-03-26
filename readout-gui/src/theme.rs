use readout_persistence::config::DashboardTheme;

pub fn apply_theme(ctx: &egui::Context, theme: DashboardTheme) {
    match theme {
        DashboardTheme::Dark => ctx.set_visuals(egui::Visuals::dark()),
        DashboardTheme::Light => ctx.set_visuals(egui::Visuals::light()),
        DashboardTheme::System => {
            // egui doesn't detect OS theme directly; default to dark
            ctx.set_visuals(egui::Visuals::dark());
        }
    }
}
