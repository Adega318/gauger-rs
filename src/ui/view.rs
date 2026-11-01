use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

use crate::app::App;
use crate::model::{Health, State};

pub(crate) fn render(f: &mut ratatui::Frame, app: &App) {
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
