use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::app::{App, RuntimeStatus};

use super::style;

pub(crate) fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(area);

    render_header(frame, header_area, app);
    render_table(frame, body_area, app);
    render_footer(frame, footer_area);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let containers = app.visible_len();
    let runtimes = app.runtimes();

    let mut spans = vec![
        Span::styled("gauger", style::title()),
        Span::styled(format!(" v{}", env!("CARGO_PKG_VERSION")), style::muted()),
        Span::styled(" │ ", style::muted()),
        Span::styled(containers.to_string(), style::highlight()),
        Span::raw(format!(
            " container{}",
            if containers == 1 { "" } else { "s" }
        )),
        Span::styled(" │ ", style::muted()),
    ];

    for (runtime, status) in runtimes {
        let (indicator, runtime_style) = match status {
            RuntimeStatus::Up => ("●", style::success()),
            RuntimeStatus::Down => ("✖", style::danger()),
        };

        spans.extend([
            Span::styled(indicator, runtime_style),
            Span::raw(" "),
            Span::styled(runtime.to_string(), runtime_style),
            Span::raw("  "),
        ]);
    }

    let line = Line::from(spans);

    frame.render_widget(Paragraph::new(line).alignment(Alignment::Left), area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let rows = app
        .container_rows()
        .into_iter()
        .map(|(runtime, container)| {
            Row::new([
                Cell::from(runtime.to_string()).style(style::runtime(runtime)),
                Cell::from(container.name.clone()).style(style::bold()),
                Cell::from(container.id.clone()).style(style::muted()),
                Cell::from(container.state.to_string()).style(style::state(container.state)),
                Cell::from(container.health.to_string()).style(style::health(container.health)),
            ])
        });

    let widths = [
        Constraint::Length(10), // Runtime
        Constraint::Min(20),    // Name
        Constraint::Length(12), // ID
        Constraint::Length(10), // State
        Constraint::Length(10), // Health
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(["RUNTIME", "NAME", "ID", "STATE", "HEALTH"])
                .style(style::header())
                .bottom_margin(1),
        )
        .column_spacing(2)
        .row_highlight_style(style::selected())
        .highlight_symbol(Span::styled("› ", style::highlight()));

    let mut state = TableState::default();
    state.select(
        (app.visible_len() > 0).then_some(app.selected.min(app.visible_len().saturating_sub(1))),
    );

    frame.render_stateful_widget(table, area, &mut state);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let shortcuts = [
        ("q", "quit"),
        ("r", "refresh"),
        ("g/G", "jump"),
        ("j/k", "navigate"),
        ("Ctrl+u/d", "page"),
    ];

    let spans = shortcuts.into_iter().flat_map(|(key, description)| {
        [
            Span::styled(format!(" {key} "), style::shortcut()),
            Span::styled(format!("{description}  "), style::muted()),
        ]
    });

    let footer = Paragraph::new(Line::from_iter(spans)).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(style::border()),
    );

    frame.render_widget(footer, area);
}
