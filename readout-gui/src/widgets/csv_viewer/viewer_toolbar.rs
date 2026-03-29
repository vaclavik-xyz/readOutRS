use super::data_store::CsvDataStore;
use super::{InteractionMode, ViewerAction};
use egui::RichText;
use egui_phosphor::regular as icons;

pub fn show(
    ui: &mut egui::Ui,
    data_store: &mut CsvDataStore,
    current_mode: InteractionMode,
    following: bool,
) -> ViewerAction {
    let mut action = ViewerAction::None;

    ui.horizontal_wrapped(|ui| {
        if ui
            .button(RichText::new(format!("{} Open", icons::FOLDER_OPEN)).small())
            .clicked()
        {
            action = ViewerAction::OpenFile;
        }
        if ui
            .button(RichText::new(format!("{} Add", icons::PLUS)).small())
            .clicked()
        {
            action = ViewerAction::AddFile;
        }

        ui.separator();

        if ui
            .button(RichText::new(format!("{} Fit", icons::ARROWS_OUT)).small())
            .clicked()
        {
            action = ViewerAction::ZoomFit;
        }

        if let Some(next_action) =
            action_from_mode_button(ui, current_mode, InteractionMode::Measure, "Measure")
        {
            action = next_action;
        }
        if let Some(next_action) =
            action_from_mode_button(ui, current_mode, InteractionMode::Select, "Select")
        {
            action = next_action;
        }
        if let Some(next_action) =
            action_from_mode_button(ui, current_mode, InteractionMode::Marker, "Marker")
        {
            action = next_action;
        }

        ui.menu_button("Modes", |ui| {
            let modes = data_store.all_modes();
            if modes.is_empty() {
                ui.label(RichText::new("No modes loaded").small().weak());
                return;
            }

            for mode in modes {
                let mut visible = data_store.is_mode_visible(&mode);
                if ui.checkbox(&mut visible, mode.as_str()).changed() {
                    data_store.set_mode_visible(&mode, visible);
                }
            }
        });

        ui.separator();

        if ui
            .button(RichText::new(format!("{} Export", icons::EXPORT)).small())
            .clicked()
        {
            action = ViewerAction::Export;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if data_store.files().iter().any(|file| file.is_live) {
                let (label, color) = if following {
                    ("Live · Following", egui::Color32::from_rgb(110, 210, 140))
                } else {
                    ("Live · Paused", egui::Color32::from_rgb(240, 190, 90))
                };
                if ui
                    .button(RichText::new(label).small().color(color))
                    .on_hover_text("Toggle auto-follow")
                    .clicked()
                {
                    action = ViewerAction::ToggleFollow;
                }
            }

            for (file_idx, file) in data_store.files().iter().enumerate().rev() {
                let name = file
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("CSV");

                let response = ui
                    .selectable_label(
                        file.visible,
                        RichText::new(format!("● {name}")).small().color(file.color),
                    )
                    .on_hover_text(if file.visible {
                        "Hide series"
                    } else {
                        "Show series"
                    });
                if response.clicked() {
                    action = ViewerAction::ToggleFileVisibility(file_idx);
                }
                response.context_menu(|ui| {
                    if ui.button("Remove").clicked() {
                        action = ViewerAction::RemoveFile(file_idx);
                        ui.close();
                    }
                });
            }
        });
    });

    action
}

fn action_from_mode_button(
    ui: &mut egui::Ui,
    current_mode: InteractionMode,
    target_mode: InteractionMode,
    label: &str,
) -> Option<ViewerAction> {
    if ui
        .selectable_label(current_mode == target_mode, label)
        .clicked()
    {
        Some(ViewerAction::SetMode(if current_mode == target_mode {
            InteractionMode::Normal
        } else {
            target_mode
        }))
    } else {
        None
    }
}
