use std::collections::BTreeMap;
use std::time::Duration;

use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{Event, EventStream, KeyEventKind};
use tokio::time::MissedTickBehavior;

use self::app::{App, AppError, Effect, RuntimeState};
use self::cli::Cli;
use self::collector::{Collector, CollectorHandle};
use self::model::Runtime;
use self::runtime::RuntimeClient;
use self::runtime::docker::DockerRuntime;
use self::runtime::podman::PodmanRuntime;

mod app;
mod cli;
mod collector;
mod headless;
mod model;
mod runtime;
mod ui;

const TICK_RATE: Duration = Duration::from_millis(250);

#[tokio::main]
async fn main() {
    if let Err(err) = run_main().await {
        eprintln!("gauger: {err}");
        let code = if matches!(err, AppError::Usage(_)) {
            2
        } else {
            1
        };
        std::process::exit(code);
    }
}

async fn run_main() -> Result<(), AppError> {
    match Cli::parse(std::env::args().skip(1))
        .map_err(|err| AppError::Usage(format!("{err}\n\n{}", Cli::usage())))?
    {
        Cli::Help => {
            print!("{}", Cli::usage());
            Ok(())
        }
        Cli::Version => {
            println!("gauger {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Cli::Tui => run_tui().await,
        Cli::NoTui => run_headless().await,
    }
}

async fn run_tui() -> Result<(), AppError> {
    let (clients, runtimes) = connect_runtimes()?;
    let mut app = App::new(runtimes);
    let collector = Collector::new(clients).spawn();

    let mut terminal = ratatui::try_init()?;
    let result = run(&mut terminal, &mut app, collector).await;
    ratatui::restore();
    result
}

async fn run_headless() -> Result<(), AppError> {
    let (clients, runtimes) = connect_runtimes()?;
    let collector = Collector::new(clients).spawn();
    headless::run(collector, &runtimes).await
}

fn connect_runtimes() -> Result<(Vec<RuntimeClient>, BTreeMap<Runtime, RuntimeState>), AppError> {
    let mut clients = Vec::new();
    let mut runtimes = BTreeMap::new();

    match DockerRuntime::connect() {
        Ok(client) => {
            clients.push(client);
            runtimes.insert(Runtime::Docker, RuntimeState::connecting());
        }
        Err(err) => {
            runtimes.insert(Runtime::Docker, RuntimeState::unavailable(err.to_string()));
        }
    }

    match PodmanRuntime::connect() {
        Ok(client) => {
            clients.push(client);
            runtimes.insert(Runtime::Podman, RuntimeState::connecting());
        }
        Err(err) => {
            runtimes.insert(Runtime::Podman, RuntimeState::unavailable(err.to_string()));
        }
    }

    if clients.is_empty() {
        return Err(AppError::NoRuntime);
    }

    Ok((clients, runtimes))
}

async fn run(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    mut collector: CollectorHandle,
) -> Result<(), AppError> {
    let mut input = EventStream::new();
    let mut input_open = true;
    let mut collector_open = true;
    let mut tick = tokio::time::interval(TICK_RATE);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        tokio::select! {
            biased;

            event = collector.recv(), if collector_open => {
                match event {
                    Some(event) => app.apply(event),
                    None => {
                        collector_open = false;
                        app.collector_stopped();
                    }
                }
            }

            event = input.next(), if input_open => {
                match event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        app.handle_key(key);
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return Err(err.into()),
                    None => {
                        input_open = false;
                        app.input_stopped();
                    }
                }
            }

            _ = tick.tick() => app.tick(),
        }

        for effect in app.drain_effects() {
            match effect {
                Effect::Refresh(runtime) => collector.refresh(runtime),
                Effect::RefreshAll => collector.refresh_all(),
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
