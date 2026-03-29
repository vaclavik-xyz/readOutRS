use super::data_store::CsvDataStore;
use super::{InteractionMode, ViewerAction};
use egui::RichText;
use egui_phosphor::regular as icons;

pub fn show(
    ui: &mut egui::Ui,
    data_store: &CsvDataStore,
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
                    ("LIVE", egui::Color32::from_rgb(110, 210, 140))
                } else {
                    ("PAUSED", egui::Color32::from_rgb(240, 190, 90))
                };
                if ui
                    .button(RichText::new(label).small().color(color))
                    .on_hover_text("Toggle live polling")
                    .clicked()
                {
                    action = ViewerAction::ToggleFollow;
                }
            }

            for file in data_store.files().iter().rev() {
                let name = file
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("CSV");
                ui.label(RichText::new(format!("● {name}")).small().color(file.color));
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
    if ui.selectable_label(current_mode == target_mode, label).clicked() {
        Some(ViewerAction::SetMode(if current_mode == target_mode {
            InteractionMode::Normal
        } else {
            target_mode
        }))
    } else {
        None
    }
}
