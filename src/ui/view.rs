use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState,
    },
};

use crate::app::{App, RuntimeStatus};
use crate::model::ResourceUsage;

use super::{format, style};

const LABEL_WIDTH: usize = 12;

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

    let show_memory = area.width >= 76;
    let show_network = area.width >= 94;
    let show_block = area.width >= 108;
    let column_count =
        5 + usize::from(show_memory) + usize::from(show_network) + usize::from(show_block);

    let mut rows = Vec::new();
    let mut selected_row = None;
    let mut container_index = 0;

    for (project, containers) in projects {
        if !project.is_empty() {
            rows.push(
                Row::new([Cell::from(format!("▸ {project}")).column_span(column_count as u16)])
                    .style(style::project()),
            );
        }

        for (runtime, container) in containers {
            if container_index == app.selected {
                selected_row = Some(rows.len());
            }

            let usage = app.usage(runtime, &container.id);
            let mut cells = vec![
                Cell::from(runtime.to_string()).style(style::runtime(runtime)),
                Cell::from(container.name.clone()).style(style::bold()),
                cpu_cell(usage),
            ];

            if show_memory {
                cells.push(memory_cell(usage));
            }
            if show_network {
                cells.push(network_cell(usage));
            }
            if show_block {
                cells.push(block_cell(usage));
            }

            cells
                .push(Cell::from(container.state.to_string()).style(style::state(container.state)));
            cells.push(
                Cell::from(container.health.to_string()).style(style::health(container.health)),
            );

            rows.push(Row::new(cells));

            container_index += 1;
        }
    }

    let mut headers = vec!["RUNTIME", "NAME", "CPU"];
    let mut widths = vec![
        Constraint::Length(8),
        Constraint::Min(16),
        Constraint::Length(6),
    ];

    if show_memory {
        headers.push("MEM");
        widths.push(Constraint::Length(14));
    }
    if show_network {
        headers.push("NET");
        widths.push(Constraint::Length(13));
    }
    if show_block {
        headers.push("I/O");
        widths.push(Constraint::Length(12));
    }

    headers.push("STATE");
    widths.push(Constraint::Length(10));
    headers.push("HEALTH");
    widths.push(Constraint::Length(10));

    let table = Table::new(rows, widths)
        .header(Row::new(headers).style(style::header()).bottom_margin(1))
        .column_spacing(2)
        .row_highlight_style(style::selected())
        .highlight_symbol(Span::styled("› ", style::highlight()));

    if app.selected == 0 {
        *table_state.offset_mut() = 0;
    }

    table_state.select(selected_row);

    frame.render_stateful_widget(table, area, table_state);
}

fn cpu_cell(usage: Option<&ResourceUsage>) -> Cell<'static> {
    match usage.and_then(|usage| usage.cpu_percent) {
        Some(percent) => Cell::from(format!("{percent:.1}%")).style(style::usage_percent(percent)),
        None => unavailable(),
    }
}

fn memory_cell(usage: Option<&ResourceUsage>) -> Cell<'static> {
    let Some(usage) = usage else {
        return unavailable();
    };

    if usage.memory_used == 0 && usage.memory_limit == 0 {
        return unavailable();
    }

    let text = if usage.memory_limit > 0 {
        format!(
            "{}/{}",
            format::bytes_compact(usage.memory_used),
            format::bytes_compact(usage.memory_limit)
        )
    } else {
        format::bytes_compact(usage.memory_used)
    };

    let cell = Cell::from(text);

    match usage.memory_percent {
        Some(percent) => cell.style(style::usage_percent(percent)),
        None => cell.style(style::normal()),
    }
}

fn network_cell(usage: Option<&ResourceUsage>) -> Cell<'static> {
    let Some(usage) = usage else {
        return unavailable();
    };

    match (usage.network_rx_rate, usage.network_tx_rate) {
        (Some(rx), Some(tx)) => Cell::from(format!(
            "↓{} ↑{}",
            format::bytes_compact(rx),
            format::bytes_compact(tx)
        )),
        _ => unavailable(),
    }
}

fn block_cell(usage: Option<&ResourceUsage>) -> Cell<'static> {
    let Some(usage) = usage else {
        return unavailable();
    };

    match (usage.block_read_rate, usage.block_write_rate) {
        (Some(read), Some(write)) => Cell::from(format!(
            "R{} W{}",
            format::bytes_compact(read),
            format::bytes_compact(write)
        )),
        _ => unavailable(),
    }
}

fn unavailable() -> Cell<'static> {
    Cell::from("—").style(style::muted())
}

fn render_detail(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.area(), 64, 20);
    frame.render_widget(Clear, area);

    let (runtime, container) = app.container(app.selected);
    let usage = app.usage(runtime, &container.id);

    let mut text = vec![
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

    if let Some(usage) = usage {
        let cpu_style = usage
            .cpu_percent
            .map(style::usage_percent)
            .unwrap_or_else(style::muted);

        text.extend([
            Line::from(""),
            Line::from(Span::styled("Resources", style::header())),
            detail_line(
                "CPU",
                Span::styled(format::percent(usage.cpu_percent), cpu_style),
            ),
            detail_line("Memory", Span::raw(memory_detail(usage))),
            detail_line(
                "Network",
                Span::raw(format!(
                    "↓ {}  ↑ {}",
                    format::rate(usage.network_rx_rate),
                    format::rate(usage.network_tx_rate)
                )),
            ),
            detail_line(
                "Net total",
                Span::raw(format!(
                    "↓ {}  ↑ {}",
                    format::bytes(usage.network_rx_bytes),
                    format::bytes(usage.network_tx_bytes)
                )),
            ),
            detail_line(
                "Block I/O",
                Span::raw(format!(
                    "R {}  W {}",
                    format::rate(usage.block_read_rate),
                    format::rate(usage.block_write_rate)
                )),
            ),
            detail_line(
                "PIDs",
                Span::raw(
                    usage
                        .pids
                        .map(|pids| pids.to_string())
                        .unwrap_or_else(|| "—".to_owned()),
                ),
            ),
        ]);
    }

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

fn memory_detail(usage: &ResourceUsage) -> String {
    if usage.memory_limit > 0 {
        format!(
            "{} / {} ({})",
            format::bytes(usage.memory_used),
            format::bytes(usage.memory_limit),
            format::percent(usage.memory_percent)
        )
    } else {
        format::bytes(usage.memory_used)
    }
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::{Terminal, backend::TestBackend};

    use crate::{
        app::App,
        collector::CollectorEvent,
        model::{Container, Health, ResourceUsage, Runtime, State},
    };

    use super::*;

    fn app_with_stats() -> App {
        let mut app = App::new(vec![Runtime::Docker]);

        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![Container {
                id: "abc123".to_owned(),
                name: "demo".to_owned(),
                image: "alpine".to_owned(),
                runtime: Runtime::Docker,
                state: State::Running,
                health: Health::Healthy,
                compose_project: Some("demo-project".to_owned()),
            }],
        })
        .unwrap();

        app.apply_event(CollectorEvent::Stats {
            runtime: Runtime::Docker,
            samples: vec![(
                "abc123".to_owned(),
                ResourceUsage {
                    cpu_percent: Some(12.5),
                    memory_used: 100,
                    memory_limit: 200,
                    memory_percent: Some(50.0),
                    network_rx_bytes: 10,
                    network_tx_bytes: 20,
                    network_rx_rate: Some(1),
                    network_tx_rate: Some(2),
                    block_read_bytes: 30,
                    block_write_bytes: 40,
                    block_read_rate: Some(3),
                    block_write_rate: Some(4),
                    pids: Some(5),
                    sampled_at: Instant::now(),
                },
            )],
        })
        .unwrap();

        app
    }

    fn draw(app: &App, width: u16) -> String {
        let backend = TestBackend::new(width, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TableState::default();

        terminal
            .draw(|frame| render(frame, app, &mut state))
            .unwrap();

        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn detail_labels_align_values() {
        let labels = [
            "ID",
            "Name",
            "Image",
            "Runtime",
            "State",
            "Health",
            "CPU",
            "Memory",
            "Network",
            "Net total",
            "Block I/O",
            "PIDs",
        ];

        for label in labels {
            let line = detail_line(label, Span::raw("value"));
            let label_span = &line.spans[0];

            assert_eq!(
                label_span.content.chars().count(),
                LABEL_WIDTH,
                "label {label:?} should pad to the shared label column"
            );
            assert_eq!(
                label_span.content.as_ref(),
                format!("{label:<LABEL_WIDTH$}"),
                "label {label:?} should be padded consistently"
            );
        }
    }

    #[test]
    fn wide_table_shows_all_resource_columns() {
        let content = draw(&app_with_stats(), 120);

        for header in ["CPU", "MEM", "NET", "I/O"] {
            assert!(content.contains(header), "missing {header} column");
        }

        assert!(content.contains("demo-project"));
    }

    #[test]
    fn narrow_table_hides_optional_resource_columns() {
        let content = draw(&app_with_stats(), 60);

        assert!(content.contains("CPU"));
        assert!(!content.contains("MEM"));
        assert!(!content.contains("NET"));
        assert!(!content.contains("I/O"));
    }
}
