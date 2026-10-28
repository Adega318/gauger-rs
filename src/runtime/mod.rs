use bollard::Docker;
use bollard::query_parameters::ListContainersOptionsBuilder;

use crate::model::{Container, ContainerStats, Runtime};

use self::error::RuntimeError;

pub mod docker;
pub mod error;
pub mod podman;

pub struct RuntimeClient {
    runtime: Runtime,
    client: Docker,
}

impl RuntimeClient {
    pub fn runtime(&self) -> Runtime {
        self.runtime
    }

    pub async fn list_containers(&self) -> Result<Vec<Container>, RuntimeError> {
        let options = ListContainersOptionsBuilder::default().all(true).build();

        let containers = self
            .client
            .list_containers(Some(options))
            .await
            .map_err(|err| RuntimeError::RequestError {
                runtime: self.runtime.to_string(),
                reason: err.to_string(),
            })?;

        Ok(containers
            .into_iter()
            .map(|container| Container::from_summary(container, &self.runtime))
            .collect())
    }

    pub async fn container_stats(
        &self,
        container_id: &str,
    ) -> Result<ContainerStats, RuntimeError> {
        let _ = container_id;
        todo!("Fetch and normalize container stats")
    }
}
