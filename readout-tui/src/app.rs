use crate::widgets;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use readout_core::dashboard_state::DashboardState;
use readout_core::types::{ConnectionState, DeviceId, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct TuiApp {
    pub state: DashboardState,
    pub chart_state: widgets::chart::TuiChartState,
    pub settings_screen: widgets::settings::TuiSettingsScreen,
    pub config: AppConfiguration,
    pub config_path: std::path::PathBuf,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new(config: AppConfiguration, config_path: std::path::PathBuf) -> Self {
        let settings_screen = widgets::settings::TuiSettingsScreen::new(&config);
        Self {
            state: DashboardState::new(),
            chart_state: widgets::chart::TuiChartState::default(),
            settings_screen,
            config,
            config_path,
            should_quit: false,
        }
    }

    pub fn handle_event(&mut self, event: RuntimeEvent) {
        self.state.handle_event(event);
    }

    /// Returns Some(config) when settings were saved (needs async persist).
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Option<AppConfiguration> {
        if self.settings_screen.active {
            return match self.settings_screen.handle_key(key) {
                widgets::settings::SettingsAction::Save(new_config) => {
                    self.config = new_config.clone();
                    Some(new_config)
                }
                widgets::settings::SettingsAction::None => None,
            };
        }

        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('p') => self.state.paused = !self.state.paused,
            KeyCode::Char('s') => self.settings_screen.open(&self.config),
            KeyCode::Right => self.chart_state.next_range(),
            KeyCode::Left => self.chart_state.prev_range(),
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        if self.settings_screen.active {
            self.settings_screen.draw(frame, frame.area());
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(3),  // header
            Constraint::Length(8),  // device cards
            Constraint::Min(8),    // chart
            Constraint::Length(3), // status
        ])
        .split(frame.area());

        // Header with colored connection states
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Paragraph};

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

        let status_text = if self.state.paused { "PAUSED" } else { "RUNNING" };
        let status_color = if self.state.paused { Color::Yellow } else { Color::Green };

        let mut spans: Vec<Span> = vec![
            Span::raw(" readout-tui | "),
            Span::styled(
                status_text.to_string(),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
        ];
        spans.extend(connection_span("MM", self.state.connection_for(DeviceId::Multimeter)));
        spans.push(Span::raw(" | "));
        spans.extend(connection_span("USB-C", self.state.connection_for(DeviceId::UsbC)));

        let header = Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title(" readout "));
        frame.render_widget(header, chunks[0]);

        // Device cards side by side
        let card_cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        widgets::device_card::render(
            frame,
            card_cols[0],
            DeviceId::Multimeter,
            self.state.latest_measurement.get(&DeviceId::Multimeter),
            self.state.alarm_for(DeviceId::Multimeter),
        );
        widgets::device_card::render(
            frame,
            card_cols[1],
            DeviceId::UsbC,
            self.state.latest_measurement.get(&DeviceId::UsbC),
            self.state.alarm_for(DeviceId::UsbC),
        );

        // Chart
        widgets::chart::render(
            frame,
            chunks[2],
            &mut self.state.chart_pipelines,
            &mut self.chart_state,
        );

        // Status bar
        widgets::status_bar::render(
            frame,
            chunks[3],
            &self.state,
            self.chart_state.range_label(),
            self.config.use_simulator,
        );
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

pub async fn run(config: AppConfiguration, config_path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TerminalGuard::new()?;

    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;

    let cancel = CancellationToken::new();
    let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
    let _command_tx = runtime.command_sender();

    let runtime_cancel = cancel.clone();
    let runtime_handle = tokio::spawn(async move {
        runtime.run(runtime_cancel).await;
    });

    let mut app = TuiApp::new(config, config_path);
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
