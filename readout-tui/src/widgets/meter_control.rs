use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use readout_core::dashboard_state::DashboardState;
use readout_core::measurement_mode::MeasurementMode;
use readout_core::types::*;

/// Which section the cursor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterSection {
    Mode,
    Range,
    Rate,
    DualDisplay,
    Null,
    DcFilter,
    AutoImpedance,
    MathFunction,
    DbReference,
    TempSensor,
    TempUnit,
    RemoteLock,
    Reset,
}

pub struct TuiMeterControl {
    pub active: bool,
    pub cursor: usize,
    pub mode_cursor: usize,
    pub db_ref_cursor: usize,
}

impl TuiMeterControl {
    pub fn new() -> Self {
        Self {
            active: false,
            cursor: 0,
            mode_cursor: 0,
            db_ref_cursor: 8, // default 600 Ω
        }
    }

    /// Returns sections visible for the current meter mode.
    fn visible_sections(&self, state: &DashboardState) -> Vec<MeterSection> {
        let mut sections = vec![
            MeterSection::Mode,
            MeterSection::Range,
            MeterSection::Rate,
            MeterSection::DualDisplay,
            MeterSection::Null,
        ];
        if state.meter_mode == MeasurementMode::DcVoltage {
            sections.push(MeterSection::DcFilter);
            sections.push(MeterSection::AutoImpedance);
        }
        sections.push(MeterSection::MathFunction);
        if matches!(
            state.meter_math_function,
            Some(MathFunction::Db) | Some(MathFunction::Dbm)
        ) {
            sections.push(MeterSection::DbReference);
        }
        if state.meter_mode == MeasurementMode::Temperature {
            sections.push(MeterSection::TempSensor);
            sections.push(MeterSection::TempUnit);
        }
        sections.push(MeterSection::RemoteLock);
        sections.push(MeterSection::Reset);
        sections
    }

    pub fn handle_key(
        &mut self,
        key: KeyCode,
        state: &DashboardState,
    ) -> Option<MultimeterCommand> {
        let sections = self.visible_sections(state);
        if sections.is_empty() {
            return None;
        }
        self.cursor = self.cursor.min(sections.len().saturating_sub(1));
        let current = sections[self.cursor];

        match key {
            KeyCode::Esc => {
                self.active = false;
                None
            }
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                None
            }
            KeyCode::Down => {
                if self.cursor + 1 < sections.len() {
                    self.cursor += 1;
                }
                None
            }
            KeyCode::Left => self.handle_left(current, state),
            KeyCode::Right => self.handle_right(current, state),
            KeyCode::Enter | KeyCode::Char(' ') => self.handle_activate(current, state),
            _ => None,
        }
    }

    fn handle_left(
        &mut self,
        section: MeterSection,
        state: &DashboardState,
    ) -> Option<MultimeterCommand> {
        match section {
            MeterSection::Mode => {
                if self.mode_cursor > 0 {
                    self.mode_cursor -= 1;
                }
                None
            }
            MeterSection::Range => Some(MultimeterCommand::SetRange(MultimeterRange::Manual(1))),
            MeterSection::Rate => {
                let new = match state.meter_rate {
                    MultimeterRate::Medium => MultimeterRate::Fast,
                    MultimeterRate::Slow => MultimeterRate::Medium,
                    MultimeterRate::Fast => MultimeterRate::Fast,
                };
                Some(MultimeterCommand::SetRate(new))
            }
            MeterSection::DbReference => {
                if self.db_ref_cursor > 0 {
                    self.db_ref_cursor -= 1;
                }
                let ohms = DB_REFERENCE_VALUES[self.db_ref_cursor];
                Some(MultimeterCommand::SetDbReference(DbReference::Ohms(ohms)))
            }
            _ => None,
        }
    }

    fn handle_right(
        &mut self,
        section: MeterSection,
        state: &DashboardState,
    ) -> Option<MultimeterCommand> {
        match section {
            MeterSection::Mode => {
                let modes = mode_list();
                if self.mode_cursor + 1 < modes.len() {
                    self.mode_cursor += 1;
                }
                None
            }
            MeterSection::Range => Some(MultimeterCommand::SetRange(MultimeterRange::Manual(7))),
            MeterSection::Rate => {
                let new = match state.meter_rate {
                    MultimeterRate::Fast => MultimeterRate::Medium,
                    MultimeterRate::Medium => MultimeterRate::Slow,
                    MultimeterRate::Slow => MultimeterRate::Slow,
                };
                Some(MultimeterCommand::SetRate(new))
            }
            MeterSection::DbReference => {
                if self.db_ref_cursor + 1 < DB_REFERENCE_VALUES.len() {
                    self.db_ref_cursor += 1;
                }
                let ohms = DB_REFERENCE_VALUES[self.db_ref_cursor];
                Some(MultimeterCommand::SetDbReference(DbReference::Ohms(ohms)))
            }
            _ => None,
        }
    }

    fn handle_activate(
        &mut self,
        section: MeterSection,
        state: &DashboardState,
    ) -> Option<MultimeterCommand> {
        match section {
            MeterSection::Mode => {
                let modes = mode_list();
                Some(MultimeterCommand::SetMode(modes[self.mode_cursor].0))
            }
            MeterSection::Range => {
                if state.meter_auto_range {
                    Some(MultimeterCommand::SetRange(MultimeterRange::Manual(3)))
                } else {
                    Some(MultimeterCommand::SetRange(MultimeterRange::Auto))
                }
            }
            MeterSection::DualDisplay => {
                Some(MultimeterCommand::SetDualDisplay(!state.meter_dual_display))
            }
            MeterSection::Null => Some(MultimeterCommand::SetNull(!state.meter_null_enabled)),
            MeterSection::DcFilter => Some(MultimeterCommand::SetDcFilter(!state.meter_dc_filter)),
            MeterSection::AutoImpedance => Some(MultimeterCommand::SetAutoImpedance(
                !state.meter_auto_impedance,
            )),
            MeterSection::MathFunction => match state.meter_math_function {
                None => Some(MultimeterCommand::StartMath(MathFunction::Average)),
                Some(MathFunction::Average) => {
                    Some(MultimeterCommand::StartMath(MathFunction::Null))
                }
                Some(MathFunction::Null) => Some(MultimeterCommand::StartMath(MathFunction::Db)),
                Some(MathFunction::Db) => Some(MultimeterCommand::StartMath(MathFunction::Dbm)),
                Some(MathFunction::Dbm) => Some(MultimeterCommand::StopMath),
            },
            MeterSection::TempSensor => {
                Some(MultimeterCommand::SetTempSensorType(TempSensorType::Pt100))
            }
            MeterSection::TempUnit => Some(MultimeterCommand::SetTempUnit(TempUnit::Celsius)),
            MeterSection::RemoteLock => Some(MultimeterCommand::SetRemoteMode(true)),
            MeterSection::Reset => Some(MultimeterCommand::ResetDevice),
            _ => None,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &DashboardState) {
        let block = Block::default()
            .title(" Meter Control ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = self.visible_sections(state);
        let mut lines: Vec<Line> = Vec::new();

        for (i, &section) in sections.iter().enumerate() {
            let selected = i == self.cursor;
            let marker = if selected { "▸ " } else { "  " };
            let highlight = if selected {
                Color::Yellow
            } else {
                Color::White
            };

            let line = match section {
                MeterSection::Mode => {
                    let modes = mode_list();
                    let mut spans = vec![Span::styled(
                        format!("{marker}Mode:   "),
                        Style::default().fg(highlight),
                    )];
                    for (j, (mode, label)) in modes.iter().enumerate() {
                        let is_current = state.meter_mode == *mode;
                        let is_sub_selected = selected && j == self.mode_cursor;
                        let style = if is_current {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else if is_sub_selected {
                            Style::default().fg(Color::Black).bg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                    Line::from(spans)
                }
                MeterSection::Range => {
                    let label = if state.meter_auto_range {
                        "Auto".to_string()
                    } else if state.meter_range_label.is_empty() {
                        "---".to_string()
                    } else {
                        state.meter_range_label.clone()
                    };
                    Line::from(vec![
                        Span::styled(format!("{marker}Range:  "), Style::default().fg(highlight)),
                        Span::styled("◀ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            label,
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" ▶", Style::default().fg(Color::DarkGray)),
                        Span::raw("  "),
                        Span::styled(
                            if state.meter_auto_range {
                                "[Auto]"
                            } else {
                                " Auto "
                            },
                            if state.meter_auto_range {
                                Style::default().fg(Color::Black).bg(Color::Green)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        ),
                        Span::styled("  Enter=toggle", Style::default().fg(Color::DarkGray)),
                    ])
                }
                MeterSection::Rate => {
                    let mut spans = vec![Span::styled(
                        format!("{marker}Rate:   "),
                        Style::default().fg(highlight),
                    )];
                    for (rate, label) in &[
                        (MultimeterRate::Fast, "Fast"),
                        (MultimeterRate::Medium, "Medium"),
                        (MultimeterRate::Slow, "Slow"),
                    ] {
                        let is_current = state.meter_rate == *rate;
                        let style = if is_current {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                    spans.push(Span::styled(
                        "  ←/→=change",
                        Style::default().fg(Color::DarkGray),
                    ));
                    Line::from(spans)
                }
                MeterSection::DualDisplay => Line::from(vec![
                    Span::styled(format!("{marker}Dual:   "), Style::default().fg(highlight)),
                    Span::styled(
                        checkbox(state.meter_dual_display),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(" Frequency sub-display"),
                ]),
                MeterSection::Null => Line::from(vec![
                    Span::styled(format!("{marker}NULL:   "), Style::default().fg(highlight)),
                    Span::styled(
                        checkbox(state.meter_null_enabled),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(" Relative measurement"),
                ]),
                MeterSection::DcFilter => Line::from(vec![
                    Span::styled(format!("{marker}Filter: "), Style::default().fg(highlight)),
                    Span::styled(
                        checkbox(state.meter_dc_filter),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(" DC filter"),
                ]),
                MeterSection::AutoImpedance => Line::from(vec![
                    Span::styled(format!("{marker}Auto Z: "), Style::default().fg(highlight)),
                    Span::styled(
                        checkbox(state.meter_auto_impedance),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(" Auto impedance"),
                ]),
                MeterSection::MathFunction => {
                    let label = match state.meter_math_function {
                        None => "Off",
                        Some(MathFunction::Average) => "MIN/MAX",
                        Some(MathFunction::Null) => "REL",
                        Some(MathFunction::Db) => "dB",
                        Some(MathFunction::Dbm) => "dBm",
                    };
                    let mut spans = vec![
                        Span::styled(format!("{marker}Math:   "), Style::default().fg(highlight)),
                        Span::styled(
                            format!("[{label}]"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("  Enter=cycle", Style::default().fg(Color::DarkGray)),
                    ];
                    if state.meter_math_function == Some(MathFunction::Average)
                        && let Some(ref stats) = state.meter_math_stats
                    {
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(
                            format!(
                                "Min:{:.3} Max:{:.3} Avg:{:.3}",
                                stats.min, stats.max, stats.avg
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    Line::from(spans)
                }
                MeterSection::DbReference => {
                    let ohms = DB_REFERENCE_VALUES[self.db_ref_cursor];
                    Line::from(vec![
                        Span::styled(format!("{marker}dB Ref: "), Style::default().fg(highlight)),
                        Span::styled("◀ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{ohms} Ω"), Style::default().fg(Color::Cyan)),
                        Span::styled(" ▶", Style::default().fg(Color::DarkGray)),
                    ])
                }
                MeterSection::TempSensor => Line::from(vec![
                    Span::styled(format!("{marker}Sensor: "), Style::default().fg(highlight)),
                    Span::raw("KITS90 / PT100  Enter=set"),
                ]),
                MeterSection::TempUnit => Line::from(vec![
                    Span::styled(format!("{marker}Unit:   "), Style::default().fg(highlight)),
                    Span::raw("°C / °F / K  Enter=set"),
                ]),
                MeterSection::RemoteLock => Line::from(vec![
                    Span::styled(format!("{marker}Remote: "), Style::default().fg(highlight)),
                    Span::styled("Enter=Lock panel", Style::default().fg(Color::DarkGray)),
                ]),
                MeterSection::Reset => Line::from(vec![
                    Span::styled(format!("{marker}Reset:  "), Style::default().fg(highlight)),
                    Span::styled("Enter=Reset device", Style::default().fg(Color::Red)),
                ]),
            };

            lines.push(line);
        }

        // Footer
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            " [Esc] Back  [↑/↓] Navigate  [Enter/Space] Activate  [←/→] Adjust",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }
}

fn mode_list() -> &'static [(MeasurementMode, &'static str)] {
    &[
        (MeasurementMode::DcVoltage, "V DC"),
        (MeasurementMode::AcVoltage, "V AC"),
        (MeasurementMode::DcCurrent, "A DC"),
        (MeasurementMode::AcCurrent, "A AC"),
        (MeasurementMode::Resistance, "Ω"),
        (MeasurementMode::Capacitance, "Cap"),
        (MeasurementMode::Frequency, "Hz"),
        (MeasurementMode::Diode, "Diod"),
        (MeasurementMode::Continuity, "Cont"),
        (MeasurementMode::Temperature, "Temp"),
        (MeasurementMode::Period, "Per"),
    ]
}

fn checkbox(enabled: bool) -> &'static str {
    if enabled { "[x]" } else { "[ ]" }
}
