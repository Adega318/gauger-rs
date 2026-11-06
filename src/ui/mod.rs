use std::collections::HashSet;
use std::io;
use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode,
};
use futures_util::{FutureExt, StreamExt};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::TableState;
use tokio::time::MissedTickBehavior;

use crate::app::{App, Effect};
use crate::collector::Collector;
use crate::model::Runtime;
use crate::runtime::{RuntimeClient, connect};

use self::error::UiError;
use self::view::render;

pub mod error;
mod format;
mod style;
mod view;

const TICK_RATE: Duration = Duration::from_millis(250);

pub(crate) async fn run_tui(clients: Vec<RuntimeClient>) -> Result<(), UiError> {
    let mut watched: HashSet<Runtime> = clients.iter().map(RuntimeClient::runtime).collect();

    let mut app = App::new(vec![Runtime::Docker, Runtime::Podman]);
    let mut collector = Collector::new(clients).spawn();

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tick = tokio::time::interval(TICK_RATE);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut input = EventStream::new();
    let mut table_state = TableState::default();

    let result = (async {
        loop {
            tokio::select! {
                Some(event) = collector.recv() => {
                    if let Err(err) = app.apply_event(event) {
                        eprintln!("gauger: {err}");
                    }
                }

                event = input.next() => {
                    let mut batching = is_resize(&event);
                    handle_input(&mut app,event)?;
                    while batching && let Some(event) = input.next().now_or_never(){
                        batching = is_resize(&event);
                        handle_input(&mut app, event)?;
                    }
                }

                _ = tick.tick() => app.tick()
            }

            for effect in app.drain_effects() {
                match effect {
                    Effect::Refresh => {
                        for runtime in app.down_runtimes() {
                            if watched.contains(&runtime) {
                                continue;
                            }
                            if let Ok(client) = connect(runtime) {
                                watched.insert(runtime);
                                collector.add_runtime(client);
                            }
                        }
                        collector.refresh_all();
                    }
                }
            }

            if app.should_quit {
                break Ok(());
            }

            execute!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
            terminal.draw(|f| render(f, &app, &mut table_state))?;
            execute!(terminal.backend_mut(), EndSynchronizedUpdate)?;
        }
    })
    .await;

    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = terminal.show_cursor();

    result
}

fn is_resize(event: &Option<Result<Event, std::io::Error>>) -> bool {
    matches!(event, Some(Ok(Event::Resize(_, _))))
}

fn handle_input(app: &mut App, event: Option<Result<Event, io::Error>>) -> Result<(), UiError> {
    match event {
        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
            app.handle_key(key);
        }
        Some(Ok(Event::Resize(_, _))) => {}
        Some(Ok(_)) => {}
        Some(Err(err)) => return Err(err.into()),
        None => {}
    }

    Ok(())
}
