use std::collections::HashMap;
use std::time::Duration;

use bollard::Docker;
use bollard::errors::Error;
use bollard::plugin::{EventMessage, EventMessageTypeEnum};
use bollard::query_parameters::{EventsOptionsBuilder, ListContainersOptionsBuilder};
use futures_util::stream::BoxStream;

use crate::model::{Action, Container, ContainerEvent, Health, Runtime};

use self::error::RuntimeError;

pub mod docker;
pub mod error;
pub mod podman;

const CONNECT_MIN_BACKOFF: Duration = Duration::from_secs(1);
const CONNECT_MAX_BACKOFF: Duration = Duration::from_secs(30);
const CONNECT_MAX_ATTEMPTS: u32 = 5;

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

pub async fn connect_with_retry(runtime: Runtime) -> Result<RuntimeClient, RuntimeError> {
    let mut backoff = CONNECT_MIN_BACKOFF;
    let mut last_err = None;

    for attempt in 1..=CONNECT_MAX_ATTEMPTS {
        match connect(runtime) {
            Ok(client) => return Ok(client),
            Err(err) => {
                last_err = Some(err);
                if attempt == CONNECT_MAX_ATTEMPTS {
                    break;
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(CONNECT_MAX_BACKOFF);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| RuntimeError::ConnectionError {
        runtime: runtime.to_string(),
        reason: "failed to connect after maximum attempts".to_owned(),
    }))
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
    use super::*;

    async fn engine_is_reachable(runtime: Runtime) -> bool {
        connect(runtime).is_ok()
    }

    async fn assert_connect_with_retry_succeeds(runtime: Runtime) {
        if !engine_is_reachable(runtime).await {
            return; // engine not available in this environment; nothing to assert
        }

        let client = connect_with_retry(runtime)
            .await
            .unwrap_or_else(|err| panic!("connect_with_retry({runtime:?}) failed: {err}"));

        assert_eq!(
            client.runtime(),
            runtime,
            "returned client should target {runtime:?}"
        );
    }

    #[tokio::test]
    async fn connect_with_retry_succeeds_for_reachable_docker() {
        assert_connect_with_retry_succeeds(Runtime::Docker).await;
    }

    #[tokio::test]
    async fn connect_with_retry_succeeds_for_reachable_podman() {
        assert_connect_with_retry_succeeds(Runtime::Podman).await;
    }
}
