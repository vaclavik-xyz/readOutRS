use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::LogLevel;

pub struct TuiLogPanel {
    pub visible: bool,
    pub scroll_offset: usize,
}

impl TuiLogPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &DashboardState) {
        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = state
            .log_entries
            .iter()
            .rev()
            .skip(self.scroll_offset)
            .take(inner.height as usize)
            .map(|entry| {
                let (prefix, color) = match entry.level {
                    LogLevel::Error => ("ERR", Color::Red),
                    LogLevel::Warning => ("WRN", Color::Yellow),
                    LogLevel::Info => ("INF", Color::Green),
                    LogLevel::Debug => ("DBG", Color::DarkGray),
                };
                Line::from(vec![
                    Span::styled(format!("[{prefix}] "), Style::default().fg(color)),
                    Span::raw(entry.message.clone()),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }
}
