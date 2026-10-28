use bollard::Docker;

use crate::model::Runtime;

use super::RuntimeClient;
use super::error::RuntimeError;

pub struct DockerRuntime;

impl DockerRuntime {
    pub fn connect() -> Result<RuntimeClient, RuntimeError> {
        let client =
            Docker::connect_with_local_defaults().map_err(|err| RuntimeError::ConnectionError {
                runtime: Runtime::Docker.to_string(),
                reason: err.to_string(),
            })?;

        Ok(RuntimeClient {
            runtime: Runtime::Docker,
            client,
        })
    }
}
