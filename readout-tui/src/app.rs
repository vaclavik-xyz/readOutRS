use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use readout_core::dashboard_state::DashboardState;
use readout_core::types::{Command, RuntimeEvent};
use readout_io::runtime::Runtime;
use readout_persistence::config::AppConfiguration;
use ratatui::Frame;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct TuiApp {
    pub state: DashboardState,
    pub config: AppConfiguration,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new(config: AppConfiguration) -> Self {
        Self {
            state: DashboardState::new(),
            config,
            should_quit: false,
        }
    }

    pub fn handle_event(&mut self, event: RuntimeEvent) {
        self.state.handle_event(event);
    }

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('p') => self.state.paused = !self.state.paused,
            _ => {}
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        use ratatui::layout::{Constraint, Layout};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let chunks = Layout::vertical([
            Constraint::Length(3), // header
            Constraint::Min(5),   // body
            Constraint::Length(3), // status
        ])
        .split(frame.area());

        // Header
        let header = Paragraph::new(format!(
            " readout-tui | {} | {}",
            if self.state.paused {
                "PAUSED"
            } else {
                "RUNNING"
            },
            format!(
                "MM: {:?} | USB-C: {:?}",
                self.state
                    .connection_for(readout_core::types::DeviceId::Multimeter),
                self.state
                    .connection_for(readout_core::types::DeviceId::UsbC),
            ),
        ))
        .block(Block::default().borders(Borders::ALL).title(" readout "));
        frame.render_widget(header, chunks[0]);

        // Body placeholder
        let body = Paragraph::new("Dashboard widgets — Task 23")
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(body, chunks[1]);

        // Status
        let status = Paragraph::new(format!(
            " Measurements: {} | Errors: {} | [q]uit [p]ause",
            self.state.health.measurement_count, self.state.health.error_count,
        ))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(status, chunks[2]);
    }
}

pub async fn run(config: AppConfiguration) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let cancel = CancellationToken::new();
    let (runtime, mut broadcast_rx) = Runtime::new(config.clone());
    let _command_tx = runtime.command_sender();

    let runtime_cancel = cancel.clone();
    tokio::spawn(async move {
        runtime.run(runtime_cancel).await;
    });

    let mut app = TuiApp::new(config);
    let mut render_interval = tokio::time::interval(Duration::from_millis(50)); // ~20 FPS

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

        // Draw
        terminal.draw(|frame| app.draw(frame))?;

        if app.should_quit {
            break;
        }

        // Wait for next event or render tick
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = render_interval.tick() => {
                // Check for keyboard input (non-blocking)
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        app.handle_key(key.code, key.modifiers);
                    }
                }
            }
        }
    }

    // Cleanup
    cancel.cancel();
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
