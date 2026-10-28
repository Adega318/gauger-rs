use bollard::Docker;

use crate::model::Runtime;

use super::RuntimeClient;
use super::error::RuntimeError;

pub struct PodmanRuntime;

impl PodmanRuntime {
    pub fn connect() -> Result<RuntimeClient, RuntimeError> {
        let client =
            Docker::connect_with_unix("/run/podman/podman.sock", 120, bollard::API_DEFAULT_VERSION)
                .map_err(|err| RuntimeError::ConnectionError {
                    runtime: Runtime::Podman.to_string(),
                    reason: err.to_string(),
                })?;

        Ok(RuntimeClient {
            runtime: Runtime::Podman,
            client,
        })
    }
}
