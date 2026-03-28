use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, is_simulator: bool) {
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
        Span::raw(" "),
        Span::styled("[q]uit ", Style::default().fg(Color::DarkGray)),
        Span::styled("[p]ause ", Style::default().fg(Color::DarkGray)),
        Span::styled("[c]trl ", Style::default().fg(Color::DarkGray)),
        Span::styled("[s]ett ", Style::default().fg(Color::DarkGray)),
        Span::styled("[l]og ", Style::default().fg(Color::DarkGray)),
        Span::styled("[m]etric ", Style::default().fg(Color::DarkGray)),
        Span::styled("[e]rst ", Style::default().fg(Color::DarkGray)),
        Span::styled("[1]mm ", Style::default().fg(Color::DarkGray)),
        Span::styled("[2]usb ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "[\u{2190}\u{2192}]rng",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let status = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(status, area);
}
