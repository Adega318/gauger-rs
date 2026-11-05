use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState,
    },
};

use crate::app::{App, RuntimeStatus};

use super::style;

const LABEL_WIDTH: usize = 9;

pub(crate) fn render(frame: &mut Frame, app: &App, table_state: &mut TableState) {
    let area = frame.area();

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(area);

    render_header(frame, header_area, app);
    render_table(frame, body_area, app, table_state);
    render_footer(frame, footer_area);

    if app.show_detail {
        render_detail(frame, app);
    }
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

fn render_table(frame: &mut Frame, area: Rect, app: &App, table_state: &mut TableState) {
    let projects = app.container_rows();

    let mut rows = Vec::new();
    let mut selected_row = None;
    let mut container_index = 0;

    for (project, containers) in projects {
        if !project.is_empty() {
            rows.push(Row::new([Cell::from(format!("▸ {project}"))]).style(style::project()));
        }

        for (runtime, container) in containers {
            if container_index == app.selected {
                selected_row = Some(rows.len());
            }

            rows.push(Row::new([
                Cell::from(runtime.to_string()).style(style::runtime(runtime)),
                Cell::from(container.name.clone()).style(style::bold()),
                Cell::from(container.id.clone()).style(style::muted()),
                Cell::from(container.state.to_string()).style(style::state(container.state)),
                Cell::from(container.health.to_string()).style(style::health(container.health)),
            ]));

            container_index += 1;
        }
    }

    let widths = [
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
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

    if app.selected == 0 {
        *table_state.offset_mut() = 0;
    }

    table_state.select(selected_row);

    frame.render_stateful_widget(table, area, table_state);
}

fn render_detail(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.area(), 50, 10);
    frame.render_widget(Clear, area);

    let (_, container) = app.container(app.selected);

    let text = vec![
        detail_line(
            "ID",
            Span::styled(container.id.to_string(), style::normal()),
        ),
        detail_line(
            "Name",
            Span::styled(container.name.to_string(), style::normal()),
        ),
        detail_line(
            "Image",
            Span::styled(container.image.to_string(), style::normal()),
        ),
        detail_line(
            "Runtime",
            Span::styled(
                container.runtime.to_string(),
                style::runtime(container.runtime),
            ),
        ),
        detail_line(
            "State",
            Span::styled(container.state.to_string(), style::state(container.state)),
        ),
        detail_line(
            "Health",
            Span::styled(
                container.health.to_string(),
                style::health(container.health),
            ),
        ),
    ];

    let popup = Paragraph::new(text).block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(style::border())
            .title(format!(" {} Details ", container.name))
            .title_style(style::title())
            .padding(Padding::new(2, 2, 1, 1)),
    );

    frame.render_widget(popup, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let shortcuts = [
        ("q", "quit"),
        ("r", "refresh"),
        ("enter", "details"),
        ("g/G", "jump"),
        ("j/k", "navigate"),
        ("Ctrl+u/d", "page"),
    ];

    let spans = shortcut_spans(&shortcuts);
    let footer = Paragraph::new(Line::from_iter(spans)).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(style::border()),
    );

    frame.render_widget(footer, area);
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);

    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);

    area
}

fn detail_line<'a>(label: &str, value: impl Into<Span<'a>>) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<LABEL_WIDTH$}"), style::header()),
        value.into(),
    ])
}

fn shortcut_spans<'a>(shortcuts: &[(&'a str, &'a str)]) -> Vec<Span<'a>> {
    shortcuts
        .iter()
        .flat_map(|(key, description)| {
            [
                Span::styled(format!(" {key} "), style::shortcut()),
                Span::styled(format!("{description}  "), style::muted()),
            ]
        })
        .collect()
}
