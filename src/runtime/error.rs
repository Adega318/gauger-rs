use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to connect to runtime {runtime} with error: {reason}")]
    ConnectionError { runtime: String, reason: String },
    #[error("request to {runtime} runtine failed with error: {reason}")]
    RequestError { runtime: String, reason: String },
}
