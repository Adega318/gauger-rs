use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Mode};

pub mod details;
pub mod footer;
pub mod header;
pub mod help;
pub mod table;
pub mod theme;

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new(format!(
                "Terminal too small\nminimum {MIN_WIDTH}×{MIN_HEIGHT}"
            ))
            .alignment(Alignment::Center)
            .block(Block::bordered().title(" Gauger ")),
            area,
        );
        return;
    }

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    header::render(frame, header_area, app);
    table::render(frame, body_area, app);
    footer::render(frame, footer_area, app);

    match app.mode.clone() {
        Mode::Details(key) => details::render(frame, area, app, &key),
        Mode::Help => help::render(frame, area),
        Mode::Normal | Mode::Filter => {}
    }
}

pub fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub fn duration_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();

    if seconds < 2 {
        "now".to_owned()
    } else if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else {
        format!("{}h ago", seconds / 3600)
    }
}

pub fn duration_short(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();

    if seconds < 2 {
        "now".to_owned()
    } else if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h", seconds / 3600)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::{ContainerKey, RuntimeState};
    use crate::collector::CollectorEvent;
    use crate::model::{Container, Health, Runtime, State};

    fn sample_app() -> App {
        let mut app = App::new(BTreeMap::from([
            (Runtime::Docker, RuntimeState::connecting()),
            (Runtime::Podman, RuntimeState::connecting()),
        ]));

        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![Container {
                id: "05ae208348f4f09389f027adb597b16a1".to_owned(),
                name: "postgres".to_owned(),
                image: "postgres:18".to_owned(),
                runtime: Runtime::Docker,
                state: State::Running,
                health: Health::Healthy,
            }],
        });
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Podman,
            containers: vec![Container {
                id: "c4ac2a2a18fe8d9a9a6bbcbc4f2031".to_owned(),
                name: "spark".to_owned(),
                image: "spark:python3".to_owned(),
                runtime: Runtime::Podman,
                state: State::Exited,
                health: Health::Unknown,
            }],
        });

        app
    }

    fn screen(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();

        terminal
            .backend()
            .buffer()
            .content()
            .chunks(width as usize)
            .map(|cells| cells.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_header_and_container_rows() {
        let mut app = sample_app();
        let screen = screen(&mut app, 120, 24);

        assert!(screen.contains("Gauger"));
        assert!(screen.contains("postgres"));
        assert!(screen.contains("spark"));
        assert!(screen.contains("● docker"));
        assert!(screen.contains("postgres:18"));
    }

    #[test]
    fn hides_image_column_on_narrow_terminals() {
        let mut app = sample_app();
        let screen = screen(&mut app, 70, 20);

        assert!(screen.contains("postgres"));
        assert!(!screen.contains("postgres:18"));
    }

    #[test]
    fn renders_details_modal() {
        let mut app = sample_app();
        app.mode = Mode::Details((
            Runtime::Docker,
            "05ae208348f4f09389f027adb597b16a1".to_owned(),
        ));

        let screen = screen(&mut app, 120, 30);

        assert!(screen.contains("image"));
        assert!(screen.contains("postgres:18"));
        assert!(screen.contains("runtime"));
    }

    #[test]
    fn renders_help_modal() {
        let mut app = sample_app();
        app.mode = Mode::Help;

        let screen = screen(&mut app, 120, 30);

        assert!(screen.contains("Help"));
        assert!(screen.contains("refresh"));
        assert!(screen.contains("filter"));
    }

    #[test]
    fn renders_too_small_message() {
        let mut app = sample_app();
        let screen = screen(&mut app, 40, 8);

        assert!(screen.contains("Terminal too small"));
        assert!(screen.contains("minimum 60"));
    }

    #[test]
    fn renders_empty_state() {
        let mut app = App::new(BTreeMap::from([(
            Runtime::Docker,
            RuntimeState::connecting(),
        )]));
        let screen = screen(&mut app, 100, 20);

        assert!(screen.contains("No containers"));
    }

    #[test]
    fn renders_filter_prompt() {
        let mut app = sample_app();
        app.mode = Mode::Filter;
        app.filter.input = "post".to_owned();
        app.filter.applied = "post".to_owned();

        let screen = screen(&mut app, 120, 20);

        assert!(screen.contains("Filter:"));
        assert!(screen.contains("post"));
    }

    #[test]
    fn details_key_matches_snapshot_key() {
        let app = sample_app();
        let key: ContainerKey = (Runtime::Podman, "c4ac2a2a18fe8d9a9a6bbcbc4f2031".to_owned());

        assert!(app.containers.contains_key(&key));
    }
}
