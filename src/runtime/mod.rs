use std::collections::HashMap;

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
