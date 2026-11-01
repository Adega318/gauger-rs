use std::io;

use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use tokio::time::MissedTickBehavior;

use crate::app::{App, Effect};
use crate::collector::Collector;
use crate::error::GaugerError;
use crate::model::{Health, State};
use crate::runtime::RuntimeClient;
use crate::{TICK_RATE, connect_runtimes};

pub(crate) async fn run_tui() -> Result<(), GaugerError> {
    let clients = connect_runtimes()?;
    let runtimes = clients.iter().map(RuntimeClient::runtime).collect();
    let mut app = App::new(runtimes);
    let mut collector = Collector::new(clients).spawn();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tick = tokio::time::interval(TICK_RATE);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut input = EventStream::new();

    let result = (async {
        loop {
            tokio::select! {
                Some(event) = collector.recv() => {
                    if let Err(err) = app.apply_event(event) {
                        eprintln!("gauger: {err}");
                    }
                }

                event = input.next() => {
                    match event {
                        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                            app.handle_key(key);
                        }
                        Some(Ok(Event::Resize(_, _))) => {}
                        Some(Ok(_)) => {}
                        Some(Err(err)) => return Err(err.into()),
                        None => {}
                    }
                }

                _ = tick.tick() => {
                    app.request_refresh();
                    app.tick();
                }
            }

            for effect in app.drain_effects() {
                match effect {
                    Effect::Refresh => collector.refresh_all(),
                }
            }

            if app.should_quit {
                break Ok(());
            }

            terminal
                .draw(|f| render(f, &app))
                .map_err(|e| GaugerError::Io(io::Error::other(e.to_string())))?;
        }
    })
    .await;

    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = terminal.show_cursor();

    result
}

fn state_style(state: State) -> Style {
    match state {
        State::Running => Style::default().fg(Color::Green),
        State::Exited | State::Dead => Style::default().fg(Color::Red),
        State::Paused | State::Stopping | State::Removing => Style::default().fg(Color::Yellow),
        State::Created | State::Restarting => Style::default().fg(Color::Cyan),
        State::Empty | State::Unknown => Style::default().fg(Color::Gray),
    }
}

fn health_style(health: Health) -> Style {
    match health {
        Health::Healthy => Style::default().fg(Color::Green),
        Health::Unhealthy => Style::default().fg(Color::Red),
        Health::Starting => Style::default().fg(Color::Yellow),
        Health::Empty | Health::Unknown => Style::default().fg(Color::Gray),
    }
}

fn render(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header line 1: app title + counts
    let up_count = app.runtimes_up_count();
    let header_text = format!(
        "gauger {} - {} container{} across {} runtime{}",
        env!("CARGO_PKG_VERSION"),
        app.visible_len(),
        if app.visible_len() == 1 { "" } else { "s" },
        up_count,
        if up_count == 1 { "" } else { "s" },
    );
    let header = Paragraph::new(header_text);
    f.render_widget(header, chunks[0]);

    // Body: table of containers with colored State and Health cells
    let header_cells = ["Runtime", "Name", "ID", "State", "Health"];
    let rows: Vec<Row> = app
        .container_rows()
        .into_iter()
        .map(|(runtime, container)| {
            Row::new(vec![
                Cell::from(runtime.to_string()),
                Cell::from(container.name.clone()),
                Cell::from(container.id.clone()),
                Cell::from(container.state.to_string()).style(state_style(container.state)),
                Cell::from(container.health.to_string()).style(health_style(container.health)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Min(22),
        Constraint::Min(12),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(header_cells)
                .style(Style::default().bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().reversed())
        .highlight_symbol(">> ");

    let mut table_state = TableState::default();
    let selected = if app.visible_len() == 0 {
        None
    } else {
        Some(app.selected.min(app.visible_len() - 1))
    };
    table_state.select(selected);
    f.render_stateful_widget(table, chunks[2], &mut table_state);

    // Footer
    let footer = Paragraph::new("q: quit  r: refresh  g/G: jump  j/k: navigate  Ctrl+u/d: page")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);
}
