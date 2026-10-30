use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};
use crate::ui::theme;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let line = if matches!(&app.mode, Mode::Filter) {
        Line::from(vec![
            Span::styled(" Filter: ", theme::accent()),
            Span::styled(app.filter.input.clone(), theme::title()),
            Span::styled("▏", theme::accent()),
            Span::styled("  enter apply · esc cancel · ctrl+u clear", theme::muted()),
        ])
    } else if let Some(notice) = &app.notice {
        let style = if notice.error {
            theme::error()
        } else {
            theme::accent()
        };
        Line::from(Span::styled(format!(" {}", notice.message), style))
    } else {
        Line::from(vec![
            Span::styled(" q ", theme::key()),
            Span::raw("quit · "),
            Span::styled("/ ", theme::key()),
            Span::raw("filter · "),
            Span::styled("s ", theme::key()),
            Span::raw("sort · "),
            Span::styled("r ", theme::key()),
            Span::raw("refresh · "),
            Span::styled("enter ", theme::key()),
            Span::raw("details · "),
            Span::styled("? ", theme::key()),
            Span::raw("help"),
        ])
    };

    frame.render_widget(Paragraph::new(line), area);
}
