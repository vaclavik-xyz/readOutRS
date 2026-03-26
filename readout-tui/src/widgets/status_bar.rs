use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use readout_core::dashboard_state::DashboardState;

pub fn render(frame: &mut Frame, area: Rect, state: &DashboardState, range_label: &str) {
    let status = Paragraph::new(format!(
        " Measurements: {} | Errors: {} | Range: {} | [q]uit [p]ause [←/→]range [s]ettings",
        state.health.measurement_count, state.health.error_count, range_label,
    ))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(status, area);
}
