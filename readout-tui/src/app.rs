use crate::widgets;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::{ConnectionState, DeviceId, MathFunction, MultimeterRate, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Settings,
    MeterControl,
}

pub struct TuiApp {
    pub state: DashboardState,
    pub chart_state: widgets::chart::TuiChartState,
    pub settings_screen: widgets::settings::TuiSettingsScreen,
    pub meter_control: widgets::meter_control::TuiMeterControl,
    pub log_panel: widgets::log_panel::TuiLogPanel,
    pub config: AppConfiguration,
    pub config_path: std::path::PathBuf,
    pub should_quit: bool,
    pub screen: Screen,
    pub show_mm: bool,
    pub show_usbc: bool,
    pub command_tx: tokio::sync::mpsc::Sender<readout_core::types::Command>,
    pub update_check: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
}

impl TuiApp {
    pub fn new(
        config: AppConfiguration,
        config_path: std::path::PathBuf,
        command_tx: tokio::sync::mpsc::Sender<readout_core::types::Command>,
    ) -> Self {
        let settings_screen = widgets::settings::TuiSettingsScreen::new(&config);
        let show_mm = config.show_mm;
        let show_usbc = config.show_usbc;
        Self {
            state: DashboardState::new(),
            chart_state: widgets::chart::TuiChartState::default(),
            settings_screen,
            meter_control: widgets::meter_control::TuiMeterControl::new(),
            log_panel: widgets::log_panel::TuiLogPanel::new(),
            config,
            config_path,
            should_quit: false,
            screen: Screen::Dashboard,
            show_mm,
            show_usbc,
            command_tx,
            update_check: None,
        }
    }

    pub fn send_meter_command(&self, cmd: readout_core::types::MultimeterCommand) {
        let _ = self
            .command_tx
            .try_send(readout_core::types::Command::Meter(cmd));
    }

    pub fn send_command(&self, cmd: readout_core::types::Command) {
        let _ = self.command_tx.try_send(cmd);
    }

    pub fn handle_event(&mut self, event: RuntimeEvent) {
        self.state.handle_event(event);
    }

    /// Returns Some(config) when settings were saved (needs async persist).
    pub fn handle_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<AppConfiguration> {
        match self.screen {
            Screen::Dashboard => self.handle_dashboard_key(key, modifiers),
            Screen::Settings => self.handle_settings_key(key),
            Screen::MeterControl => {
                self.handle_meter_control_key(key);
                None
            }
        }
    }

    fn handle_dashboard_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<AppConfiguration> {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('p') => self.state.paused = !self.state.paused,
            KeyCode::Char('s') => {
                self.settings_screen.open(&self.config);
                self.screen = Screen::Settings;
            }
            KeyCode::Char('c') => {
                self.meter_control.active = true;
                self.screen = Screen::MeterControl;
            }
            KeyCode::Char('l') => self.log_panel.toggle(),
            KeyCode::Char('m') => self.chart_state.next_usbc_metric(),
            KeyCode::Char('1') => self.show_mm = !self.show_mm,
            KeyCode::Char('2') => self.show_usbc = !self.show_usbc,
            KeyCode::Char('e') => {
                self.send_command(readout_core::types::Command::ResetEnergy {
                    device: DeviceId::UsbC,
                });
            }
            KeyCode::Right => self.chart_state.next_range(),
            KeyCode::Left => self.chart_state.prev_range(),
            KeyCode::PageUp if self.log_panel.visible => self.log_panel.scroll_up(),
            KeyCode::PageDown if self.log_panel.visible => self.log_panel.scroll_down(),
            _ => {}
        }
        None
    }

    fn handle_settings_key(&mut self, key: KeyCode) -> Option<AppConfiguration> {
        let result = match self.settings_screen.handle_key(key) {
            widgets::settings::SettingsAction::Save(new_config) => {
                self.config = new_config.clone();
                Some(new_config)
            }
            widgets::settings::SettingsAction::None => None,
        };
        if !self.settings_screen.active {
            self.screen = Screen::Dashboard;
        }
        result
    }

    fn handle_meter_control_key(&mut self, key: KeyCode) {
        if let Some(cmd) = self.meter_control.handle_key(key, &self.state) {
            self.send_meter_command(cmd);
        }
        if !self.meter_control.active {
            self.screen = Screen::Dashboard;
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match self.screen {
            Screen::Dashboard => self.draw_dashboard(frame),
            Screen::Settings => {
                self.settings_screen
                    .draw(frame, frame.area(), &self.state.update_available)
            }
            Screen::MeterControl => {
                self.meter_control.draw(frame, frame.area(), &self.state);
            }
        }
    }

    fn draw_dashboard(&mut self, frame: &mut Frame) {
        let card_height = if self.show_mm || self.show_usbc { 8 } else { 0 };
        let chunks = Layout::vertical([
            Constraint::Length(3),           // header
            Constraint::Length(card_height), // device cards
            Constraint::Min(8),              // chart
            Constraint::Length(3),           // status
        ])
        .split(frame.area());

        // Header with colored connection states
        fn connection_span(label: &str, state: &ConnectionState) -> Vec<Span<'static>> {
            let (text, color) = match state {
                ConnectionState::Connected => ("Connected", Color::Green),
                ConnectionState::Connecting => ("Connecting", Color::Yellow),
                ConnectionState::Reconnecting => ("Reconnecting", Color::Yellow),
                ConnectionState::Disconnected => ("Disconnected", Color::DarkGray),
                ConnectionState::Error(e) => {
                    return vec![
                        Span::raw(format!("{label}: ")),
                        Span::styled(
                            format!("Error({e})"),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                    ];
                }
            };
            vec![
                Span::raw(format!("{label}: ")),
                Span::styled(text.to_string(), Style::default().fg(color)),
            ]
        }

        let status_text = if self.state.paused {
            "PAUSED"
        } else {
            "RUNNING"
        };
        let status_color = if self.state.paused {
            Color::Yellow
        } else {
            Color::Green
        };

        let mut spans: Vec<Span> = vec![
            Span::raw(" readOut | "),
            Span::styled(
                status_text.to_string(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
        ];
        spans.extend(connection_span(
            "MM",
            self.state.connection_for(DeviceId::Multimeter),
        ));
        spans.push(Span::raw(" | "));
        spans.extend(connection_span(
            "USB-C",
            self.state.connection_for(DeviceId::UsbC),
        ));
        spans.push(Span::styled(
            format!(
                " | M:{} E:{} R:{}",
                self.state.health.measurement_count,
                self.state.health.error_count,
                self.chart_state.range_label(),
            ),
            Style::default().fg(Color::DarkGray),
        ));

        let header = Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title(" readOut "));
        frame.render_widget(header, chunks[0]);

        // Device cards — layout adapts to visibility
        let mode_str = format!("{:?}", self.state.meter_mode);
        let rate_str = match self.state.meter_rate {
            MultimeterRate::Fast => "Fast",
            MultimeterRate::Medium => "Medium",
            MultimeterRate::Slow => "Slow",
        };
        let math_str = self.state.meter_math_function.map(|f| match f {
            MathFunction::Null => "REL",
            MathFunction::Average => "MIN/MAX",
            MathFunction::Db => "dB",
            MathFunction::Dbm => "dBm",
        });
        let range_label = if self.state.meter_range_label.is_empty() {
            "Auto"
        } else {
            &self.state.meter_range_label
        };

        if self.show_mm && self.show_usbc {
            let card_cols =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[1]);
            widgets::device_card::render(
                frame,
                card_cols[0],
                DeviceId::Multimeter,
                self.state.latest_measurement.get(&DeviceId::Multimeter),
                self.state.alarm_for(DeviceId::Multimeter),
                Some(&mode_str),
                Some(range_label),
                Some(rate_str),
                math_str,
            );
            widgets::device_card::render(
                frame,
                card_cols[1],
                DeviceId::UsbC,
                self.state.latest_measurement.get(&DeviceId::UsbC),
                self.state.alarm_for(DeviceId::UsbC),
                None,
                None,
                None,
                None,
            );
        } else if self.show_mm {
            widgets::device_card::render(
                frame,
                chunks[1],
                DeviceId::Multimeter,
                self.state.latest_measurement.get(&DeviceId::Multimeter),
                self.state.alarm_for(DeviceId::Multimeter),
                Some(&mode_str),
                Some(range_label),
                Some(rate_str),
                math_str,
            );
        } else if self.show_usbc {
            widgets::device_card::render(
                frame,
                chunks[1],
                DeviceId::UsbC,
                self.state.latest_measurement.get(&DeviceId::UsbC),
                self.state.alarm_for(DeviceId::UsbC),
                None,
                None,
                None,
                None,
            );
        }

        // Chart (and optional log panel)
        if self.log_panel.visible {
            let chart_and_log =
                Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(chunks[2]);
            widgets::chart::render(
                frame,
                chart_and_log[0],
                &mut self.state.chart_pipelines,
                &mut self.state.usbc_chart_pipelines,
                &mut self.chart_state,
            );
            self.log_panel.draw(frame, chart_and_log[1], &self.state);
        } else {
            widgets::chart::render(
                frame,
                chunks[2],
                &mut self.state.chart_pipelines,
                &mut self.state.usbc_chart_pipelines,
                &mut self.chart_state,
            );
        }

        // Status bar
        widgets::status_bar::render(frame, chunks[3], self.config.use_simulator);
    }
}

/// RAII guard that restores terminal state on drop.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        crossterm::terminal::enable_raw_mode()?;
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        ) {
            let _ = crossterm::terminal::disable_raw_mode();
            return Err(e.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
    }
}

pub async fn run(
    config: AppConfiguration,
    config_path: std::path::PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TerminalGuard::new()?;

    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;

    let cancel = CancellationToken::new();
    let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
    let command_tx = runtime.command_sender();

    let runtime_cancel = cancel.clone();
    let runtime_handle = tokio::spawn(async move {
        runtime.run(runtime_cancel).await;
    });

    let mut app = TuiApp::new(config.clone(), config_path, command_tx);

    // Check for updates in background
    if config.check_for_updates {
        let update_result = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let r = update_result.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(version) = readout_core::update_checker::check_for_update() {
                *r.lock().unwrap() = Some(version);
            }
        });
        app.update_check = Some(update_result);
    }

    let mut render_interval = tokio::time::interval(Duration::from_millis(50));
    let mut event_stream = EventStream::new();
    let mut save_handle: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        // Drain runtime events
        loop {
            match broadcast_rx.try_recv() {
                Ok(event) => app.handle_event(event),
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("TUI lagged {n} events");
                }
                Err(_) => break,
            }
        }

        // Poll update check result
        if app.state.update_available.is_none()
            && let Some(ref check) = app.update_check
            && let Ok(guard) = check.try_lock()
            && let Some(ref version) = *guard
        {
            app.state.update_available = Some(version.clone());
        }

        terminal.draw(|frame| app.draw(frame))?;

        if app.should_quit {
            break;
        }

        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = render_interval.tick() => {}
            event = event_stream.next() => {
                match event {
                    Some(Ok(Event::Key(key))) => {
                        if let Some(new_config) = app.handle_key(key.code, key.modifiers) {
                            let path = app.config_path.clone();
                            save_handle = Some(tokio::task::spawn_blocking(move || {
                                if let Err(e) = readout_persistence::config_store::save(&new_config, &path) {
                                    tracing::error!("Failed to save config: {e:?}");
                                }
                            }));
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        tracing::warn!("terminal event error: {e}");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    // Wait for pending config save before shutdown (with timeout)
    if let Some(h) = save_handle.take() {
        let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
    }

    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), runtime_handle).await;

    Ok(())
}
