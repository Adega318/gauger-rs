use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table};

use crate::app::{App, RuntimeStatus, short_id};
use crate::model::Container;
use crate::ui::theme;

const MIN_WIDTH_FOR_IMAGE: u16 = 80;
const MIN_WIDTH_FOR_ID: u16 = 100;

struct Column {
    label: &'static str,
    width: Constraint,
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let title = format!(
        " containers · {} · {} {} ",
        app.visible.len(),
        app.sort.key.label(),
        if app.sort.descending { "↓" } else { "↑" }
    );
    let block = Block::bordered().title(title);

    if app.visible.is_empty() {
        let message = if app.filter.is_active() {
            format!("No containers match \"{}\"", app.filter.applied)
        } else {
            "No containers".to_owned()
        };
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .block(block)
                .style(theme::muted()),
            area,
        );
        return;
    }

    let show_id = area.width >= MIN_WIDTH_FOR_ID;
    let show_image = area.width >= MIN_WIDTH_FOR_IMAGE;
    let columns = columns(show_id, show_image);

    let header = Row::new(
        columns
            .iter()
            .map(|column| Cell::from(column.label))
            .collect::<Vec<_>>(),
    )
    .style(theme::header());

    let rows: Vec<Row<'static>> = app
        .visible
        .iter()
        .filter_map(|key| {
            let container = app.containers.get(key)?;
            let stale = app.runtimes.get(&container.runtime).is_some_and(|state| {
                matches!(
                    &state.status,
                    RuntimeStatus::Down | RuntimeStatus::Unavailable { .. }
                )
            });
            Some(container_row(container, show_id, show_image, stale))
        })
        .collect();

    let widths: Vec<Constraint> = columns.iter().map(|column| column.width).collect();

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1)
        .row_highlight_style(theme::selection())
        .highlight_symbol("▶ ");

    app.table_state.select(Some(app.selected));
    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn columns(show_id: bool, show_image: bool) -> Vec<Column> {
    let mut columns = Vec::new();

    if show_id {
        columns.push(Column {
            label: "ID",
            width: Constraint::Length(12),
        });
    }

    columns.extend([
        Column {
            label: "NAME",
            width: Constraint::Fill(1),
        },
        Column {
            label: "RUNTIME",
            width: Constraint::Length(8),
        },
        Column {
            label: "STATE",
            width: Constraint::Length(10),
        },
        Column {
            label: "HEALTH",
            width: Constraint::Length(9),
        },
    ]);

    if show_image {
        columns.push(Column {
            label: "IMAGE",
            width: Constraint::Length(30),
        });
    }

    columns
}

fn container_row(
    container: &Container,
    show_id: bool,
    show_image: bool,
    stale: bool,
) -> Row<'static> {
    let mut cells = Vec::new();

    if show_id {
        cells.push(Cell::from(short_id(&container.id)).style(theme::muted()));
    }

    cells.push(Cell::from(container.name.clone()));
    cells.push(Cell::from(container.runtime.to_string()).style(theme::runtime(container.runtime)));
    cells.push(Cell::from(container.state.to_string()).style(theme::state(container.state)));
    cells.push(Cell::from(container.health.to_string()).style(theme::health(container.health)));

    if show_image {
        cells.push(Cell::from(container.image.clone()).style(theme::muted()));
    }

    let row = Row::new(cells);
    if stale {
        row.style(theme::stale())
    } else {
        row
    }
}
