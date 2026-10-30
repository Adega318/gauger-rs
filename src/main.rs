use self::cli::Cli;
use self::collector::{Collector, CollectorEvent};
use self::error::GaugerError;
use self::runtime::RuntimeClient;
use self::runtime::docker::DockerRuntime;
use self::runtime::podman::PodmanRuntime;

mod app;
mod cli;
mod collector;
mod error;
mod model;
mod runtime;

#[tokio::main]
async fn main() {
    if let Err(err) = run_main().await {
        let code = err.exit_code();
        eprintln!("gauger: {err}");
        std::process::exit(code);
    }
}

async fn run_main() -> Result<(), GaugerError> {
    match Cli::parse(std::env::args().skip(1))? {
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

async fn run_tui() -> Result<(), GaugerError> {
    todo!()
}
async fn run_headless() -> Result<(), GaugerError> {
    let clients = connect_runtimes()?;

    let mut collector = Collector::new(clients).spawn();

    while let Some(event) = collector.recv().await {
        match event {
            CollectorEvent::Snapshot {
                runtime,
                containers,
            } => {
                println!(
                    "Snapshot of {runtime}:{}",
                    if containers.is_empty() {
                        " (empty)"
                    } else {
                        ""
                    }
                );

                for container in &containers {
                    println!("  {container}");
                }
            }
            CollectorEvent::Container(event) => {
                println!("Container: {}", event);
            }
            CollectorEvent::RuntimeUp { runtime } => {
                println!("up: {}", runtime)
            }
            CollectorEvent::RuntimeDown { runtime, reason } => {
                println!("down: {}, {}", runtime, reason)
            }
        }
    }

    Ok(())
}

fn connect_runtimes() -> Result<Vec<RuntimeClient>, GaugerError> {
    let mut clients = Vec::new();

    if let Ok(client) = DockerRuntime::connect() {
        clients.push(client);
    }

    if let Ok(client) = PodmanRuntime::connect() {
        clients.push(client);
    }

    Ok(clients)
}
