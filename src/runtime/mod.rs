use std::collections::HashMap;

use bollard::Docker;
use bollard::errors::Error;
use bollard::models::ContainerStatsResponse;
use bollard::plugin::{EventMessage, EventMessageTypeEnum};
use bollard::query_parameters::{
    EventsOptionsBuilder, ListContainersOptionsBuilder, StatsOptionsBuilder,
};
use futures_util::stream::{BoxStream, StreamExt};

use crate::model::{Action, Container, ContainerEvent, Health, Runtime};

use self::error::RuntimeError;

pub mod docker;
pub mod error;
pub mod podman;

#[derive(Clone)]
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
            .map(|container| Container::from_summary(container, self.runtime))
            .collect())
    }

    /// Fetch a single, immediate stats sample for a container.
    ///
    /// `one_shot` avoids Docker's default two-cycle wait; rates are computed
    /// locally from consecutive samples instead.
    pub async fn stats_once(&self, id: &str) -> Result<ContainerStatsResponse, RuntimeError> {
        let options = StatsOptionsBuilder::default()
            .stream(false)
            .one_shot(true)
            .build();

        let mut stream = self.client.stats(id, Some(options));

        match stream.next().await {
            Some(Ok(stats)) => Ok(stats),
            Some(Err(err)) => Err(RuntimeError::RequestError {
                runtime: self.runtime.to_string(),
                reason: err.to_string(),
            }),
            None => Err(RuntimeError::RequestError {
                runtime: self.runtime.to_string(),
                reason: "stats stream returned no samples".to_owned(),
            }),
        }
    }

    pub fn get_event_stream(&self) -> BoxStream<'_, Result<EventMessage, Error>> {
        let mut filters: HashMap<String, Vec<String>> = HashMap::new();
        filters.insert("type".into(), vec!["container".into()]);

        let options = EventsOptionsBuilder::default().filters(&filters).build();
        Box::pin(self.client.events(Some(options)))
    }

    pub fn normalize_event(&self, event: EventMessage) -> Option<ContainerEvent> {
        if !matches!(event.typ, Some(EventMessageTypeEnum::CONTAINER)) {
            return None;
        }

        let action = Action::from(event.action.unwrap_or_default());
        if is_noise(&action) {
            return None;
        }

        let actor = event.actor.unwrap_or_default();
        let id = actor.id?;
        let name = actor
            .attributes
            .and_then(|attributes| attributes.get("name").cloned());

        Some(ContainerEvent {
            runtime: self.runtime,
            id,
            name,
            action,
        })
    }

    pub async fn inspect_health(&self, id: &str) -> Result<Health, RuntimeError> {
        let inspect = self
            .client
            .inspect_container(id, None)
            .await
            .map_err(|err| RuntimeError::RequestError {
                runtime: self.runtime.to_string(),
                reason: err.to_string(),
            })?;

        let status = inspect
            .state
            .and_then(|state| state.health)
            .and_then(|health| health.status);

        Ok(Health::from(status))
    }
}

pub fn connect(runtime: Runtime) -> Result<RuntimeClient, RuntimeError> {
    match runtime {
        Runtime::Docker => docker::DockerRuntime::connect(),
        Runtime::Podman => podman::PodmanRuntime::connect(),
    }
}

fn is_noise(action: &Action) -> bool {
    let Action::Unknown(raw) = action else {
        return false;
    };

    // Both engines emit high-volume events for probes and maintenance that do
    // not affect container state.
    raw.starts_with("exec")
        || matches!(
            raw.as_str(),
            "attach"
                | "detach"
                | "copy"
                | "export"
                | "top"
                | "resize"
                | "update"
                | "mount"
                | "unmount"
                | "sync"
                | "init"
                | "cleanup"
                | "commit"
                | "prune"
        )
}

#[cfg(test)]
mod tests {
    use crate::model::State;

    use super::*;

    #[tokio::test]
    async fn stats_once_returns_a_sample_for_a_running_container() {
        for runtime in [Runtime::Docker, Runtime::Podman] {
            let Ok(client) = connect(runtime) else {
                continue;
            };

            let Ok(containers) = client.list_containers().await else {
                continue;
            };

            let Some(container) = containers
                .into_iter()
                .find(|container| container.state == State::Running)
            else {
                continue;
            };

            let stats = client.stats_once(&container.id).await;
            assert!(
                stats.is_ok(),
                "stats request for {runtime:?} container {} should succeed: {stats:?}",
                container.id
            );
        }
    }

    #[test]
    fn connect_succeeds_for_reachable_runtime() {
        for runtime in [Runtime::Docker, Runtime::Podman] {
            let Ok(client) = connect(runtime) else {
                continue; // engine not available in this environment; nothing to assert
            };

            assert_eq!(
                client.runtime(),
                runtime,
                "returned client should target {runtime:?}"
            );
        }
    }
}
