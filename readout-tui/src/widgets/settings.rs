use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use readout_persistence::config::AppConfiguration;

pub struct TuiSettingsScreen {
    pub active: bool,
    pub fields: Vec<SettingsField>,
    pub selected: usize,
    pub editing: bool,
    draft: AppConfiguration,
}

pub struct SettingsField {
    pub label: &'static str,
    pub value: String,
    field_kind: FieldKind,
}

enum FieldKind {
    Bool(fn(&mut AppConfiguration) -> &mut bool),
    U32(fn(&mut AppConfiguration) -> &mut u32),
    String(fn(&mut AppConfiguration) -> &mut String),
}

impl TuiSettingsScreen {
    pub fn new(config: &AppConfiguration) -> Self {
        let mut s = Self {
            active: false,
            fields: Vec::new(),
            selected: 0,
            editing: false,
            draft: config.clone(),
        };
        s.rebuild_fields();
        s
    }

    pub fn open(&mut self, config: &AppConfiguration) {
        self.draft = config.clone();
        self.rebuild_fields();
        self.selected = 0;
        self.editing = false;
        self.active = true;
    }

    fn rebuild_fields(&mut self) {
        self.fields = vec![
            SettingsField {
                label: "Simulator mode",
                value: self.draft.use_simulator.to_string(),
                field_kind: FieldKind::Bool(|c| &mut c.use_simulator),
            },
            SettingsField {
                label: "Multimeter enabled",
                value: self.draft.multimeter_enabled.to_string(),
                field_kind: FieldKind::Bool(|c| &mut c.multimeter_enabled),
            },
            SettingsField {
                label: "Multimeter port",
                value: self.draft.multimeter_port.clone(),
                field_kind: FieldKind::String(|c| &mut c.multimeter_port),
            },
            SettingsField {
                label: "USB-C enabled",
                value: self.draft.usbc_enabled.to_string(),
                field_kind: FieldKind::Bool(|c| &mut c.usbc_enabled),
            },
            SettingsField {
                label: "USB-C port",
                value: self.draft.usbc_port.clone(),
                field_kind: FieldKind::String(|c| &mut c.usbc_port),
            },
            SettingsField {
                label: "Sample rate (Hz)",
                value: self.draft.sample_rate_hz.to_string(),
                field_kind: FieldKind::U32(|c| &mut c.sample_rate_hz),
            },
        ];
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) -> SettingsAction {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Esc => {
                if self.editing {
                    self.editing = false;
                } else {
                    self.active = false;
                }
                SettingsAction::None
            }
            KeyCode::Up => {
                if !self.editing && self.selected > 0 {
                    self.selected -= 1;
                }
                SettingsAction::None
            }
            KeyCode::Down => {
                if !self.editing && self.selected < self.fields.len().saturating_sub(1) {
                    self.selected += 1;
                }
                SettingsAction::None
            }
            KeyCode::Enter => {
                if self.editing {
                    // Apply edit to draft
                    self.apply_field_at(self.selected);
                    self.editing = false;
                } else {
                    // Toggle bools immediately, start editing for others
                    match self.fields[self.selected].field_kind {
                        FieldKind::Bool(accessor) => {
                            let val = accessor(&mut self.draft);
                            *val = !*val;
                            self.fields[self.selected].value = val.to_string();
                        }
                        _ => {
                            self.editing = true;
                        }
                    }
                }
                SettingsAction::None
            }
            KeyCode::Char(c) if self.editing => {
                self.fields[self.selected].value.push(c);
                SettingsAction::None
            }
            KeyCode::Backspace if self.editing => {
                self.fields[self.selected].value.pop();
                SettingsAction::None
            }
            KeyCode::Char('s') if !self.editing => {
                self.apply_all_fields_to_draft();
                self.draft.clamp_values();
                self.active = false;
                SettingsAction::Save(self.draft.clone())
            }
            _ => SettingsAction::None,
        }
    }

    fn apply_field_at(&mut self, index: usize) {
        let field = &self.fields[index];
        match field.field_kind {
            FieldKind::Bool(accessor) => {
                let val = accessor(&mut self.draft);
                *val = field.value == "true";
            }
            FieldKind::U32(accessor) => {
                if let Ok(v) = field.value.parse() {
                    let val = accessor(&mut self.draft);
                    *val = v;
                }
            }
            FieldKind::String(accessor) => {
                let val = accessor(&mut self.draft);
                *val = field.value.clone();
            }
        }
    }

    fn apply_all_fields_to_draft(&mut self) {
        for i in 0..self.fields.len() {
            self.apply_field_at(i);
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Settings — [↑↓] navigate [Enter] edit/toggle [s] save [Esc] cancel ");

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let constraints: Vec<Constraint> = self
            .fields
            .iter()
            .map(|_| Constraint::Length(1))
            .collect();

        let rows = Layout::vertical(constraints).split(inner);

        for (i, field) in self.fields.iter().enumerate() {
            let selected = i == self.selected;
            let editing = selected && self.editing;

            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cursor = if editing { "▎" } else { "" };
            let line = Line::from(vec![
                Span::styled(
                    format!("{:>22}: ", field.label),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(format!("{}{cursor}", field.value), style),
            ]);

            if i < rows.len() {
                frame.render_widget(Paragraph::new(line), rows[i]);
            }
        }
    }
}

pub enum SettingsAction {
    None,
    Save(AppConfiguration),
}
