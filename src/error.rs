use thiserror::Error;

use crate::app::error::AppError;
use crate::cli::CliError;
use crate::runtime::error::RuntimeError;
use crate::ui::error::UiError;

#[derive(Debug, Error)]
pub enum GaugerError {
    #[error(transparent)]
    App(#[from] AppError),
    #[error(transparent)]
    Cli(#[from] CliError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Ui(#[from] UiError),
}

impl GaugerError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Cli(CliError::UnknownArgument(_)) => 2,
            _ => 1,
        }
    }
}
