use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::ui::{self, theme};

pub fn render(frame: &mut Frame, area: Rect) {
    let popup = ui::popup(area, 70, 12);
    frame.render_widget(Clear, popup);

    let lines = vec![
        hint("j / k, ↑ / ↓", "move"),
        hint("g / G", "first / last"),
        hint("ctrl+u / ctrl+d", "page up / down"),
        hint("/ , c", "filter / clear"),
        hint("s / S", "sort / reverse"),
        hint("r", "refresh"),
        hint("enter / ?", "details / help"),
        Line::from(Span::styled("esc / enter / q close", theme::muted())),
    ];

    let block = Block::bordered().title(Span::styled(" Help ", theme::title()));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn hint(keys: &'static str, description: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {keys:<16}"), theme::key()),
        Span::raw(description),
    ])
}
