use std::time::Duration;

use self::cli::Cli;
use self::error::GaugerError;
use self::runtime::RuntimeClient;
use self::runtime::docker::DockerRuntime;
use self::runtime::podman::PodmanRuntime;

mod app;
mod cli;
mod collector;
mod error;
mod headless;
mod model;
mod runtime;
mod ui;

const TICK_RATE: Duration = Duration::from_millis(250);

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
        Cli::Tui => ui::run_tui().await,
        Cli::NoTui => headless::run_headless().await,
    }
}

pub fn connect_runtimes() -> Result<Vec<RuntimeClient>, GaugerError> {
    let mut clients = Vec::new();

    if let Ok(client) = DockerRuntime::connect() {
        clients.push(client);
    }

    if let Ok(client) = PodmanRuntime::connect() {
        clients.push(client);
    }

    Ok(clients)
}
