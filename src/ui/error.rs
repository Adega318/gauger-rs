use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
