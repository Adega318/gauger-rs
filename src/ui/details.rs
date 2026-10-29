use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::{App, ContainerKey, RuntimeStatus};
use crate::model::Action;
use crate::ui::{self, duration_label, theme};

pub fn render(frame: &mut Frame, area: Rect, app: &App, key: &ContainerKey) {
    let Some(container) = app.containers.get(key) else {
        return;
    };

    let popup = ui::popup(area, 80, 14);
    frame.render_widget(Clear, popup);

    let mut lines = vec![
        field("id", container.id.clone()),
        field("image", container.image.clone()),
        field("runtime", container.runtime.to_string()),
        field("state", format!("{:?}", container.state)),
        field("health", format!("{:?}", container.health)),
    ];

    if let Some((last_key, action, at)) = &app.last_event
        && last_key == key
    {
        lines.push(field(
            "last",
            format!(
                "{} · {}",
                action_label(action),
                duration_label(at.elapsed())
            ),
        ));
    }

    if let Some(state) = app.runtimes.get(&container.runtime)
        && matches!(
            &state.status,
            RuntimeStatus::Down { .. } | RuntimeStatus::Unavailable { .. }
        )
    {
        lines.push(Line::from(Span::styled(
            "runtime disconnected — values may be stale",
            theme::warning(),
        )));
    }

    lines.push(Line::from(Span::styled(
        "esc / enter / q close",
        theme::muted(),
    )));

    let block = Block::bordered().title(Span::styled(
        format!(" {} ", container.name),
        theme::title(),
    ));

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn field(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<8}"), theme::muted()),
        Span::raw(value),
    ])
}

fn action_label(action: &Action) -> String {
    match action {
        Action::Create => "create".to_owned(),
        Action::Start => "start".to_owned(),
        Action::Stop => "stop".to_owned(),
        Action::Die => "die".to_owned(),
        Action::Pause => "pause".to_owned(),
        Action::Unpause => "unpause".to_owned(),
        Action::Restart => "restart".to_owned(),
        Action::Remove => "remove".to_owned(),
        Action::Rename => "rename".to_owned(),
        Action::Oom => "oom".to_owned(),
        Action::Health(health) => format!("health: {health:?}").to_lowercase(),
        Action::Unknown(raw) => raw.clone(),
    }
}
