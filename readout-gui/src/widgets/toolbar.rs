use std::time::Duration;

pub const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

pub struct ToolbarState {
    pub show_mm: bool,
    pub show_usbc: bool,
    pub paused: bool,
    pub pc_beep_enabled: bool,
    pub meter_beep_enabled: bool,
    pub selected_range_idx: usize,
    pub show_log: bool,
    pub always_on_top: bool,
}

#[derive(Default)]
pub enum ToolbarAction {
    #[default]
    None,
    TogglePause,
    TogglePcBeep,
    ToggleMeterBeep,
    SetTimeRange(usize),
    OpenSettings,
    ToggleLog,
    ToggleAlwaysOnTop,
}

pub fn show(ui: &mut egui::Ui, state: &mut ToolbarState) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        if ui
            .selectable_label(state.show_mm, egui::RichText::new("MM").size(10.0))
            .clicked()
        {
            if !state.show_mm || state.show_usbc {
                state.show_mm = !state.show_mm;
            }
        }
        if ui
            .selectable_label(state.show_usbc, egui::RichText::new("USB-C").size(10.0))
            .clicked()
        {
            if !state.show_usbc || state.show_mm {
                state.show_usbc = !state.show_usbc;
            }
        }

        ui.separator();

        let pause_label = if state.paused { "▶" } else { "⏸" };
        if ui
            .button(egui::RichText::new(pause_label).size(10.0))
            .clicked()
        {
            action = ToolbarAction::TogglePause;
        }

        ui.separator();

        let pc_icon = if state.pc_beep_enabled { "🔊" } else { "🔇" };
        if ui
            .selectable_label(
                state.pc_beep_enabled,
                egui::RichText::new(format!("{pc_icon} PC")).size(10.0),
            )
            .clicked()
        {
            action = ToolbarAction::TogglePcBeep;
        }
        let meter_icon = if state.meter_beep_enabled { "🔔" } else { "🔇" };
        if ui
            .selectable_label(
                state.meter_beep_enabled,
                egui::RichText::new(format!("{meter_icon} M")).size(10.0),
            )
            .clicked()
        {
            action = ToolbarAction::ToggleMeterBeep;
        }

    });

    // Row 2: time range + log/settings/pin
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
            let selected = i == state.selected_range_idx;
            if ui
                .selectable_label(selected, egui::RichText::new(*label).size(10.0))
                .clicked()
            {
                action = ToolbarAction::SetTimeRange(i);
            }
        }

        ui.separator();

        if ui
            .selectable_label(state.show_log, egui::RichText::new("📋").size(10.0))
            .on_hover_text("Log")
            .clicked()
        {
            action = ToolbarAction::ToggleLog;
        }

        if ui
            .button(egui::RichText::new("⚙").size(10.0))
            .on_hover_text("Settings")
            .clicked()
        {
            action = ToolbarAction::OpenSettings;
        }

        if ui
            .selectable_label(
                state.always_on_top,
                egui::RichText::new("📌").size(10.0),
            )
            .on_hover_text("Always on top")
            .clicked()
        {
            action = ToolbarAction::ToggleAlwaysOnTop;
        }
    });

    action
}
