use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use readout_core::types::{AlarmState, DeviceId, DeviceMeasurement};
use readout_core::value_format::format_si;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    device: DeviceId,
    measurement: Option<&DeviceMeasurement>,
    alarm: AlarmState,
) {
    let title = match device {
        DeviceId::Multimeter => " Multimeter ",
        DeviceId::UsbC => " USB-C Power Meter ",
    };

    let border_color = match alarm {
        AlarmState::HighAlarm | AlarmState::LowAlarm => Color::Red,
        AlarmState::Short => Color::Yellow,
        AlarmState::Open => Color::Yellow,
        AlarmState::None => Color::White,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let mut lines = Vec::new();

    if let Some(m) = measurement {
        let value_text = m
            .primary_value
            .map(|v| format_si(v, &m.primary_unit))
            .unwrap_or_else(|| format!("OL {}", m.primary_unit));

        lines.push(Line::from(vec![
            Span::styled(
                value_text,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(Span::styled(
            &m.mode_string,
            Style::default().fg(Color::DarkGray),
        )));

        if device == DeviceId::UsbC {
            if let Some(current) = m.secondary_value {
                lines.push(Line::from(format!("{current:.3} A")));
            }
            if let Some(power) = m.power_watts {
                lines.push(Line::from(format!("{power:.2} W")));
            }
            if let Some(mwh) = m.energy_mwh {
                lines.push(Line::from(format!("{mwh:.0} mWh")));
            }
        }

        match alarm {
            AlarmState::HighAlarm => {
                lines.push(Line::from(Span::styled(
                    "⚠ HIGH ALARM",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }
            AlarmState::LowAlarm => {
                lines.push(Line::from(Span::styled(
                    "⚠ LOW ALARM",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }
            AlarmState::Short => {
                lines.push(Line::from(Span::styled(
                    "⚡ SHORT",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            AlarmState::Open => {
                lines.push(Line::from(Span::styled(
                    "⊘ OPEN",
                    Style::default().fg(Color::Yellow),
                )));
            }
            AlarmState::None => {}
        }
    } else {
        lines.push(Line::from(Span::styled(
            "---",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
