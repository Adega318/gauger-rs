use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, RuntimeState, RuntimeStatus};
use crate::model::Runtime;
use crate::ui::{duration_short, theme};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let now = Instant::now();
    let count = app.containers.len();

    let mut spans = vec![Span::styled(
        format!(" {count} container{} ", if count == 1 { "" } else { "s" }),
        theme::muted(),
    )];

    for runtime in [Runtime::Docker, Runtime::Podman] {
        spans.push(Span::styled("  ", theme::muted()));
        match app.runtimes.get(&runtime) {
            Some(state) => spans.push(badge(runtime, state, now)),
            None => spans.push(Span::styled(format!("{runtime} n/a"), theme::muted())),
        }
    }

    if app.filter.is_active() {
        spans.push(Span::styled("  ", theme::muted()));
        spans.push(Span::styled(
            format!("filter: {}", app.filter.applied),
            theme::accent(),
        ));
    }

    let block = Block::bordered().title(Span::styled(" Gauger ", theme::title()));
    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn badge(runtime: Runtime, state: &RuntimeState, now: Instant) -> Span<'static> {
    match &state.status {
        RuntimeStatus::Connecting => {
            Span::styled(format!("◌ {runtime} connecting"), theme::warning())
        }
        RuntimeStatus::Up => {
            let age = state
                .last_snapshot
                .map(|snapshot| duration_short(now.saturating_duration_since(snapshot)))
                .unwrap_or_else(|| "now".to_owned());
            Span::styled(format!("● {runtime} {age}"), theme::ok())
        }
        RuntimeStatus::Down => Span::styled(format!("✕ {runtime} down"), theme::error()),
        RuntimeStatus::Unavailable { .. } => {
            Span::styled(format!("✕ {runtime} unavailable"), theme::muted())
        }
    }
}
