use std::io;

use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::time::MissedTickBehavior;

use crate::app::{App, Effect};
use crate::collector::Collector;
use crate::error::GaugerError;
use crate::runtime::RuntimeClient;
use crate::{TICK_RATE, connect_runtimes};

use self::view::render;

mod view;
mod style;

pub(crate) async fn run_tui() -> Result<(), GaugerError> {
    let clients = connect_runtimes()?;
    let runtimes = clients.iter().map(RuntimeClient::runtime).collect();
    let mut app = App::new(runtimes);
    let mut collector = Collector::new(clients).spawn();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tick = tokio::time::interval(TICK_RATE);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut input = EventStream::new();

    let result = (async {
        loop {
            tokio::select! {
                Some(event) = collector.recv() => {
                    if let Err(err) = app.apply_event(event) {
                        eprintln!("gauger: {err}");
                    }
                }

                event = input.next() => {
                    match event {
                        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                            app.handle_key(key);
                        }
                        Some(Ok(Event::Resize(_, _))) => {}
                        Some(Ok(_)) => {}
                        Some(Err(err)) => return Err(err.into()),
                        None => {}
                    }
                }

                _ = tick.tick() => {
                    app.request_refresh();
                    app.tick();
                }
            }

            for effect in app.drain_effects() {
                match effect {
                    Effect::Refresh => collector.refresh_all(),
                }
            }

            if app.should_quit {
                break Ok(());
            }

            terminal
                .draw(|f| render(f, &app))
                .map_err(|e| GaugerError::Io(io::Error::other(e.to_string())))?;
        }
    })
    .await;

    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = terminal.show_cursor();

    result
}
