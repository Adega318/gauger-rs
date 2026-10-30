use self::cli::Cli;
use self::error::GaugerError;

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
    todo!()
}
