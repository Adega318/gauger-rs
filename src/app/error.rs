use thiserror::Error;

use crate::model::Runtime;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("container {container_id} not found in {runtime}")]
    ContainerNotFound {
        runtime: Runtime,
        container_id: String,
    },
}
