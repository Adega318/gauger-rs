use self::cli::Cli;
use self::error::GaugerError;
use self::runtime::RuntimeClient;
use self::runtime::connect_with_retry;
use crate::model::Runtime;

mod app;
mod cli;
mod collector;
mod error;
mod headless;
mod model;
mod runtime;
mod ui;

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
        Cli::Tui => ui::run_tui(connect_runtimes().await?)
            .await
            .map_err(GaugerError::Ui),
        Cli::NoTui => headless::run_headless(connect_runtimes().await?).await,
    }
}

pub async fn connect_runtimes() -> Result<Vec<RuntimeClient>, GaugerError> {
    let mut clients = Vec::new();

    for runtime in [Runtime::Docker, Runtime::Podman] {
        if let Ok(client) = connect_with_retry(runtime).await {
            clients.push(client);
        }
    }

    Ok(clients)
}
