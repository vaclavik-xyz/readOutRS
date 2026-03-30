use egui_phosphor::regular as icons;
use std::time::Duration;

pub const RANGE_OPTIONS: &[(Duration, &str)] = &[
    (Duration::from_secs(10), "10s"),
    (Duration::from_secs(30), "30s"),
    (Duration::from_secs(60), "1m"),
    (Duration::from_secs(120), "2m"),
    (Duration::from_secs(300), "5m"),
    (Duration::from_secs(600), "10m"),
    (Duration::from_secs(1800), "30m"),
    (Duration::from_secs(3600), "1h"),
];

pub const GRAPH_VIEWER_ICON: &str = icons::PRESENTATION_CHART;
pub const GRAPH_VIEWER_TOOLTIP: &str = "Graph Viewer (Cmd+L)";

#[derive(Default)]
pub enum ToolbarAction {
    #[default]
    None,
    TogglePause,
    ClearCharts,
    SetTimeRange(usize),
    OpenGraphViewer,
    OpenSettings,
    ToggleAlwaysOnTop,
    OpenMeterControl,
    ToggleShowMm,
    ToggleShowUsbc,
}

pub struct TitleBarState {
    pub always_on_top: bool,
    pub selected_range_idx: usize,
    pub show_mm: bool,
    pub show_usbc: bool,
    pub paused: bool,
}

/// Pill background with guaranteed contrast against panel fill.
fn pill_bg(visuals: &egui::Visuals) -> egui::Color32 {
    let p = visuals.panel_fill;
    let lum = (p.r() as u16 + p.g() as u16 + p.b() as u16) / 3;
    let d: i16 = if lum > 128 { -20 } else { 20 };
    egui::Color32::from_rgb(
        (p.r() as i16 + d).clamp(0, 255) as u8,
        (p.g() as i16 + d).clamp(0, 255) as u8,
        (p.b() as i16 + d).clamp(0, 255) as u8,
    )
}

/// Segmented pill — shared rounded background, items inside without own frames.
fn pill_group(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let bg = pill_bg(ui.visuals());
    let row_h = ui.spacing().interact_size.y;
    egui::Frame::new()
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(1, 0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.spacing_mut().interact_size.y = row_h;
                // Strip individual widget chrome
                ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                let r = egui::CornerRadius::same(4);
                ui.visuals_mut().widgets.hovered.corner_radius = r;
                ui.visuals_mut().widgets.active.corner_radius = r;
                ui.visuals_mut().widgets.inactive.corner_radius = r;
                add_contents(ui);
            });
        });
}

/// Thin vertical divider inside a pill group.
fn pill_divider(ui: &mut egui::Ui) {
    let h = ui.spacing().interact_size.y * 0.5;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, h), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let color = ui.visuals().widgets.noninteractive.bg_stroke.color;
        ui.painter().line_segment(
            [rect.center_top(), rect.center_bottom()],
            egui::Stroke::new(1.0, color),
        );
    }
}

/// Compact toolbar: MM USB-C 30s▼ ... ⏸ ⏹ ⚙ 📊 📌
///
/// LTR for device toggles + range. RTL for transport + utility.
/// Icons colored by state — no pill backgrounds.
pub fn show_title_bar(ui: &mut egui::Ui, state: &TitleBarState) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Uniform row height — match 16px icon buttons
        ui.spacing_mut().interact_size.y = 24.0;

        // ── Device toggles pill ──
        pill_group(ui, |ui| {
            if ui
                .selectable_label(
                    state.show_mm,
                    egui::RichText::new("MM").size(11.0).strong(),
                )
                .on_hover_text("Show/hide Multimeter")
                .clicked()
            {
                action = ToolbarAction::ToggleShowMm;
            }
            pill_divider(ui);
            if ui
                .selectable_label(
                    state.show_usbc,
                    egui::RichText::new("USB-C").size(11.0).strong(),
                )
                .on_hover_text("Show/hide USB-C")
                .clicked()
            {
                action = ToolbarAction::ToggleShowUsbc;
            }
        });

        // ── Range selector ──
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

        // ── Transport pill (play/pause left, stop right) ──
        pill_group(ui, |ui| {
            let (play_icon, play_tip) = if state.paused {
                (icons::PLAY, "Resume (Cmd+P)")
            } else {
                (icons::PAUSE, "Pause (Cmd+P)")
            };
            if ui
                .button(egui::RichText::new(play_icon).size(16.0))
                .on_hover_text(play_tip)
                .clicked()
            {
                action = ToolbarAction::TogglePause;
            }
            pill_divider(ui);
            if ui
                .button(egui::RichText::new(icons::STOP).size(16.0))
                .on_hover_text("Clear Charts (Cmd+K)")
                .clicked()
            {
                action = ToolbarAction::ClearCharts;
            }
        });

        // ── RTL: utility buttons ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            let pin_text = if state.always_on_top {
                egui::RichText::new(icons::PUSH_PIN)
                    .size(16.0)
                    .color(egui::Color32::from_rgb(0x4C, 0xAF, 0x50))
            } else {
                egui::RichText::new(icons::PUSH_PIN).size(16.0)
            };
            if ui
                .selectable_label(state.always_on_top, pin_text)
                .on_hover_text("Always on top")
                .clicked()
            {
                action = ToolbarAction::ToggleAlwaysOnTop;
            }

            if ui
                .button(egui::RichText::new(GRAPH_VIEWER_ICON).size(16.0))
                .on_hover_text(GRAPH_VIEWER_TOOLTIP)
                .clicked()
            {
                action = ToolbarAction::OpenGraphViewer;
            }

            if ui
                .button(egui::RichText::new(icons::GEAR).size(16.0))
                .on_hover_text("Settings (Cmd+,)")
                .clicked()
            {
                action = ToolbarAction::OpenSettings;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui_phosphor::regular as icons;
    use std::time::Duration;

    fn default_state() -> TitleBarState {
        TitleBarState {
            always_on_top: false,
            selected_range_idx: 1,
            show_mm: true,
            show_usbc: true,
            paused: false,
        }
    }

    /// Render toolbar in a headless egui context at a given width.
    fn render_toolbar(state: &TitleBarState, width: f32) -> egui::FullOutput {
        let ctx = egui::Context::default();
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(width, 100.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show_title_bar(ui, state);
                });
            },
        )
    }

    #[test]
    fn range_options_start_with_ten_seconds() {
        assert_eq!(RANGE_OPTIONS[0], (Duration::from_secs(10), "10s"));
    }

    #[test]
    fn graph_viewer_toolbar_button_uses_graph_viewer_copy_and_new_icon() {
        assert_eq!(GRAPH_VIEWER_TOOLTIP, "Graph Viewer (Cmd+L)");
        assert_eq!(GRAPH_VIEWER_ICON, icons::PRESENTATION_CHART);
        assert_ne!(GRAPH_VIEWER_ICON, icons::CHART_LINE);
    }

    #[test]
    fn play_pause_icons_are_distinct() {
        assert_ne!(icons::PLAY, icons::PAUSE);
        assert_ne!(icons::PLAY, icons::STOP);
    }

    /// Toolbar must render without panic for every valid state combination.
    #[test]
    fn toolbar_renders_all_state_combinations() {
        for paused in [false, true] {
            for aot in [false, true] {
                for (mm, usbc) in [(true, true), (true, false), (false, true)] {
                    for range_idx in [0, RANGE_OPTIONS.len() - 1] {
                        let state = TitleBarState {
                            always_on_top: aot,
                            selected_range_idx: range_idx,
                            show_mm: mm,
                            show_usbc: usbc,
                            paused,
                        };
                        render_toolbar(&state, 340.0);
                    }
                }
            }
        }
    }

    /// All toolbar widgets must produce paint output at standard window width.
    /// This catches the RTL-clipping regression where items were laid out but
    /// fell outside the clip rect and became invisible.
    #[test]
    fn toolbar_produces_sufficient_paint_shapes_at_standard_width() {
        let output = render_toolbar(&default_state(), 340.0);
        // 8 interactive widgets (MM, USB-C, range combo, play/pause, stop,
        // settings, graph viewer, pin) each produce at least one shape.
        assert!(
            output.shapes.len() >= 8,
            "expected ≥8 shapes for 8 toolbar widgets, got {}",
            output.shapes.len()
        );
    }

    /// Shape count must be stable across widths — wider windows must not
    /// cause widgets to disappear.
    #[test]
    fn toolbar_shape_count_stable_across_widths() {
        let narrow = render_toolbar(&default_state(), 340.0).shapes.len();
        let wide = render_toolbar(&default_state(), 600.0).shapes.len();
        // Both must produce the same number of shapes (±2 for spacer rounding)
        assert!(
            narrow.abs_diff(wide) <= 2,
            "shape count differs: narrow={narrow}, wide={wide}"
        );
    }

    /// Paused vs running state must produce different paint output
    /// (different icon rendered).
    #[test]
    fn toolbar_paint_differs_between_paused_and_running() {
        let running = render_toolbar(&default_state(), 340.0);
        let mut paused_state = default_state();
        paused_state.paused = true;
        let paused = render_toolbar(&paused_state, 340.0);
        // Different icon glyphs → different shapes
        let r_shapes = format!("{:?}", running.shapes);
        let p_shapes = format!("{:?}", paused.shapes);
        assert_ne!(r_shapes, p_shapes, "paused and running must render differently");
    }
}
