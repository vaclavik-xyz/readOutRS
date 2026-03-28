use egui_phosphor::regular as icons;
use std::time::Duration;

pub const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(30), "30s"),
    (Duration::from_secs(60), "1m"),
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

#[derive(Default)]
pub enum ToolbarAction {
    #[default]
    None,
    TogglePause,
    ClearCharts,
    SetTimeRange(usize),
    OpenSettings,
    ToggleAlwaysOnTop,
    OpenMeterControl,
    ToggleShowMm,
    ToggleShowUsbc,
}

pub struct TitleBarState {
    pub always_on_top: bool,
    pub csv_active: bool,
    pub obs_active: bool,
    pub selected_range_idx: usize,
    pub show_mm: bool,
    pub show_usbc: bool,
}

/// Minimal title bar: device toggles + status indicators + range dropdown + settings + pin.
pub fn show_title_bar(ui: &mut egui::Ui, state: &TitleBarState) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    // Make title bar draggable for window movement
    let _title_bar_response = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // Leave space for macOS traffic lights
        if cfg!(target_os = "macos") {
            ui.add_space(68.0);
        }

        if ui
            .selectable_label(state.show_mm, egui::RichText::new("MM").size(10.0).strong())
            .on_hover_text("Show/hide Multimeter")
            .clicked()
        {
            action = ToolbarAction::ToggleShowMm;
        }
        if ui
            .selectable_label(
                state.show_usbc,
                egui::RichText::new("USB-C").size(10.0).strong(),
            )
            .on_hover_text("Show/hide USB-C")
            .clicked()
        {
            action = ToolbarAction::ToggleShowUsbc;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            if ui
                .selectable_label(
                    state.always_on_top,
                    egui::RichText::new(icons::PUSH_PIN).size(14.0),
                )
                .on_hover_text("Always on top")
                .clicked()
            {
                action = ToolbarAction::ToggleAlwaysOnTop;
            }

            if ui
                .button(egui::RichText::new(icons::GEAR).size(14.0))
                .on_hover_text("Settings (Cmd+,)")
                .clicked()
            {
                action = ToolbarAction::OpenSettings;
            }

            if ui
                .button(egui::RichText::new(icons::TRASH).size(14.0))
                .on_hover_text("Clear Charts (Cmd+K)")
                .clicked()
            {
                action = ToolbarAction::ClearCharts;
            }

            // Time range dropdown
            let current_label = RANGE_OPTIONS[state.selected_range_idx].1;
            egui::ComboBox::from_id_salt("range")
                .selected_text(egui::RichText::new(current_label).size(11.0))
                .width(42.0)
                .show_ui(ui, |ui| {
                    for (i, (_, label)) in RANGE_OPTIONS.iter().enumerate() {
                        let selected = state.selected_range_idx == i;
                        if ui.selectable_label(selected, *label).clicked() {
                            action = ToolbarAction::SetTimeRange(i);
                        }
                    }
                });
        });
    });

    action
}

/// Right-click context menu with pause, clear.
pub fn context_menu(ui: &mut egui::Ui, paused: bool) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    let pause_label = if paused {
        format!("{} Resume", icons::PLAY)
    } else {
        format!("{} Pause", icons::PAUSE)
    };
    if ui.button(pause_label).clicked() {
        action = ToolbarAction::TogglePause;
        ui.close();
    }

    if ui
        .button(format!("{} Clear Charts", icons::TRASH))
        .clicked()
    {
        action = ToolbarAction::ClearCharts;
        ui.close();
    }

    action
}

/// Multimeter inline control: meter control button.
pub fn mm_inline_control(ui: &mut egui::Ui) -> ToolbarAction {
    if ui
        .selectable_label(false, egui::RichText::new(icons::FADERS).size(14.0))
        .on_hover_text("Meter Control (Cmd+M)")
        .clicked()
    {
        ToolbarAction::OpenMeterControl
    } else {
        ToolbarAction::None
    }
}
