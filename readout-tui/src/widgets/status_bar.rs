use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use readout_core::dashboard_state::DashboardState;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &DashboardState,
    range_label: &str,
    is_simulator: bool,
) {
    let mode_span = if is_simulator {
        Span::styled(
            " SIM ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " HW ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    };

    let line = Line::from(vec![
        Span::raw(" "),
        mode_span,
        Span::raw(format!(
            " Measurements: {} | Errors: {} | Range: {} | [q]uit [p]ause [\u{2190}/\u{2192}]range [s]ettings",
            state.health.measurement_count, state.health.error_count, range_label,
        )),
    ]);

    let status = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(status, area);
}
