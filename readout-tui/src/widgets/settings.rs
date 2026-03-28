use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use readout_persistence::config::AppConfiguration;

pub struct TuiSettingsScreen {
    pub active: bool,
    pub fields: Vec<SettingsField>,
    pub selected: usize,
    pub editing: bool,
    pub scroll_offset: usize,
    draft: AppConfiguration,
}

pub struct SettingsField {
    pub label: &'static str,
    pub value: String,
    field_kind: FieldKindOrSeparator,
}

enum FieldKindOrSeparator {
    Field(FieldKind),
    Separator,
}

enum FieldKind {
    Bool(fn(&mut AppConfiguration) -> &mut bool),
    U32(fn(&mut AppConfiguration) -> &mut u32),
    F64(fn(&mut AppConfiguration) -> &mut f64),
    Str(fn(&mut AppConfiguration) -> &mut String),
}

impl TuiSettingsScreen {
    pub fn new(config: &AppConfiguration) -> Self {
        let mut s = Self {
            active: false,
            fields: Vec::new(),
            selected: 0,
            editing: false,
            scroll_offset: 0,
            draft: config.clone(),
        };
        s.rebuild_fields();
        // Skip initial separator
        s.selected = s.next_field_index(0);
        s
    }

    pub fn open(&mut self, config: &AppConfiguration) {
        self.draft = config.clone();
        self.rebuild_fields();
        self.selected = self.next_field_index(0);
        self.editing = false;
        self.scroll_offset = 0;
        self.active = true;
    }

    fn is_separator(&self, index: usize) -> bool {
        matches!(
            self.fields[index].field_kind,
            FieldKindOrSeparator::Separator
        )
    }

    fn next_field_index(&self, from: usize) -> usize {
        if self.fields.is_empty() {
            return 0;
        }
        let mut i = from;
        while i < self.fields.len() && self.is_separator(i) {
            i += 1;
        }
        let i = i.min(self.fields.len().saturating_sub(1));
        if self.is_separator(i) {
            // Iterative fallback — scan backwards without recursion
            let mut j = i;
            while j > 0 && self.is_separator(j) {
                j -= 1;
            }
            return j;
        }
        i
    }

    fn prev_field_index(&self, from: usize) -> usize {
        if self.fields.is_empty() {
            return 0;
        }
        let mut i = from;
        while i > 0 && self.is_separator(i) {
            i -= 1;
        }
        if self.is_separator(i) {
            // Iterative fallback — scan forwards without recursion
            let mut j = i;
            while j < self.fields.len() && self.is_separator(j) {
                j += 1;
            }
            return j.min(self.fields.len().saturating_sub(1));
        }
        i
    }

    fn rebuild_fields(&mut self) {
        use FieldKindOrSeparator::{Field, Separator};

        macro_rules! sep {
            ($label:expr) => {
                SettingsField {
                    label: $label,
                    value: String::new(),
                    field_kind: Separator,
                }
            };
        }
        macro_rules! bool_field {
            ($label:expr, $accessor:expr) => {{
                let f: fn(&mut AppConfiguration) -> &mut bool = $accessor;
                SettingsField {
                    label: $label,
                    value: (f(&mut self.draft)).to_string(),
                    field_kind: Field(FieldKind::Bool(f)),
                }
            }};
        }
        macro_rules! u32_field {
            ($label:expr, $accessor:expr) => {{
                let f: fn(&mut AppConfiguration) -> &mut u32 = $accessor;
                SettingsField {
                    label: $label,
                    value: (f(&mut self.draft)).to_string(),
                    field_kind: Field(FieldKind::U32(f)),
                }
            }};
        }
        macro_rules! f64_field {
            ($label:expr, $accessor:expr) => {{
                let f: fn(&mut AppConfiguration) -> &mut f64 = $accessor;
                SettingsField {
                    label: $label,
                    value: format!("{}", f(&mut self.draft)),
                    field_kind: Field(FieldKind::F64(f)),
                }
            }};
        }
        macro_rules! str_field {
            ($label:expr, $accessor:expr) => {{
                let f: fn(&mut AppConfiguration) -> &mut String = $accessor;
                SettingsField {
                    label: $label,
                    value: (f(&mut self.draft)).clone(),
                    field_kind: Field(FieldKind::Str(f)),
                }
            }};
        }

        self.fields = vec![
            // ── Devices ──
            sep!("Devices"),
            bool_field!("Simulator mode", |c: &mut AppConfiguration| &mut c
                .use_simulator),
            bool_field!("Multimeter enabled", |c: &mut AppConfiguration| &mut c
                .multimeter_enabled),
            str_field!("Multimeter port", |c: &mut AppConfiguration| &mut c
                .multimeter_port),
            bool_field!("MM auto-reconnect", |c: &mut AppConfiguration| &mut c
                .multimeter_auto_reconnect),
            bool_field!("USB-C enabled", |c: &mut AppConfiguration| &mut c
                .usbc_enabled),
            str_field!("USB-C port", |c: &mut AppConfiguration| &mut c.usbc_port),
            bool_field!("USB-C auto-reconnect", |c: &mut AppConfiguration| &mut c
                .usbc_auto_reconnect),
            u32_field!("Sample rate (Hz)", |c: &mut AppConfiguration| &mut c
                .sample_rate_hz),
            // ── Display ──
            sep!("Display"),
            u32_field!("Graph history (sec)", |c: &mut AppConfiguration| &mut c
                .graph_history_seconds),
            bool_field!("Log capture", |c: &mut AppConfiguration| &mut c
                .runtime_log_capture_enabled),
            // ── Alarms ──
            sep!("Alarms"),
            bool_field!("DCV high alarm", |c: &mut AppConfiguration| &mut c
                .dcv_high_alarm_enabled),
            f64_field!("DCV high value", |c: &mut AppConfiguration| &mut c
                .dcv_high_alarm_value),
            bool_field!("DCV low alarm", |c: &mut AppConfiguration| &mut c
                .dcv_low_alarm_enabled),
            f64_field!("DCV low value", |c: &mut AppConfiguration| &mut c
                .dcv_low_alarm_value),
            f64_field!("Short threshold (Ω)", |c: &mut AppConfiguration| &mut c
                .short_threshold),
            bool_field!("Beep on alarm", |c: &mut AppConfiguration| &mut c
                .beep_on_alarm),
            bool_field!("Beep on short (PC)", |c: &mut AppConfiguration| &mut c
                .beep_on_short_pc),
            bool_field!("Beep on short (meter)", |c: &mut AppConfiguration| &mut c
                .beep_on_short_meter),
            f64_field!("Beep volume", |c: &mut AppConfiguration| &mut c
                .pc_beep_volume),
            bool_field!("Beep master", |c: &mut AppConfiguration| &mut c
                .dashboard_beep_master_enabled),
            // ── CSV Logging ──
            sep!("CSV Logging"),
            bool_field!("MM CSV logging", |c: &mut AppConfiguration| &mut c
                .multimeter_csv_logging_enabled),
            str_field!("MM CSV file", |c: &mut AppConfiguration| &mut c
                .multimeter_csv_log_file_path),
            bool_field!("USB-C CSV logging", |c: &mut AppConfiguration| &mut c
                .usbc_csv_logging_enabled),
            str_field!("USB-C CSV file", |c: &mut AppConfiguration| &mut c
                .usbc_csv_log_file_path),
            // ── OBS Output ──
            sep!("OBS Output"),
            str_field!("MM output file", |c: &mut AppConfiguration| &mut c
                .multimeter_output_file),
            str_field!("USB-C output file", |c: &mut AppConfiguration| &mut c
                .usbc_output_file),
            str_field!("MM value label", |c: &mut AppConfiguration| &mut c
                .multimeter_value_label),
            str_field!("USB-C value label", |c: &mut AppConfiguration| &mut c
                .usbc_value_label),
            // ── About ──
            sep!("About"),
            bool_field!("Check for updates", |c: &mut AppConfiguration| &mut c
                .check_for_updates),
        ];
    }

    pub fn handle_key(&mut self, key: KeyCode) -> SettingsAction {
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
                    self.selected = self.prev_field_index(self.selected - 1);
                }
                SettingsAction::None
            }
            KeyCode::Down => {
                if !self.editing && self.selected < self.fields.len().saturating_sub(1) {
                    let next = self.next_field_index(self.selected + 1);
                    if next < self.fields.len() {
                        self.selected = next;
                    }
                }
                SettingsAction::None
            }
            KeyCode::Enter => {
                if self.is_separator(self.selected) {
                    return SettingsAction::None;
                }
                if self.editing {
                    self.apply_field_at(self.selected);
                    self.editing = false;
                } else {
                    match self.fields[self.selected].field_kind {
                        FieldKindOrSeparator::Field(FieldKind::Bool(accessor)) => {
                            let val = accessor(&mut self.draft);
                            *val = !*val;
                            self.fields[self.selected].value = val.to_string();
                        }
                        FieldKindOrSeparator::Separator => {}
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
        match &field.field_kind {
            FieldKindOrSeparator::Separator => {}
            FieldKindOrSeparator::Field(kind) => match kind {
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
                FieldKind::F64(accessor) => {
                    if let Ok(v) = field.value.parse::<f64>()
                        && v.is_finite()
                    {
                        *accessor(&mut self.draft) = v;
                    }
                }
                FieldKind::Str(accessor) => {
                    let val = accessor(&mut self.draft);
                    *val = field.value.clone();
                }
            },
        }
    }

    fn apply_all_fields_to_draft(&mut self) {
        for i in 0..self.fields.len() {
            self.apply_field_at(i);
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, update_available: &Option<String>) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Settings — [↑↓] navigate [Enter] edit/toggle [s] save [Esc] cancel ");

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let visible_height = inner.height as usize;
        // Auto-scroll to keep selected visible
        let scroll = if self.selected < self.scroll_offset {
            self.selected
        } else if self.selected >= self.scroll_offset + visible_height {
            self.selected - visible_height + 1
        } else {
            self.scroll_offset
        };
        self.scroll_offset = scroll;

        let mut lines: Vec<Line> = Vec::new();

        for (i, field) in self
            .fields
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible_height)
        {
            if self.is_separator(i) {
                lines.push(Line::from(Span::styled(
                    format!("── {} ──", field.label),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                continue;
            }

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
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>24}: ", field.label),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(format!("{}{cursor}", field.value), style),
            ]));
        }

        // Version and update info (always visible at end)
        let fields_end = scroll + lines.len();
        let remaining = visible_height.saturating_sub(lines.len());
        if fields_end >= self.fields.len() && remaining >= 2 {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>24}: ", "Version"),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(readout_core::update_checker::CURRENT_VERSION),
            ]));
            if let Some(new_ver) = update_available {
                let hint = if readout_core::update_checker::is_homebrew() {
                    " — brew upgrade readout"
                } else {
                    " — github.com/vaclavik-xyz/readOutRS/releases"
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{:>24}  ", ""), Style::default()),
                    Span::styled(
                        format!("v{new_ver} available"),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(hint, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

#[allow(clippy::large_enum_variant)]
pub enum SettingsAction {
    None,
    Save(AppConfiguration),
}
