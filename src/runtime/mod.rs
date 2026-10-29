use std::collections::HashMap;

use bollard::Docker;
use bollard::errors::Error;
use bollard::plugin::EventMessage;
use bollard::query_parameters::{EventsOptionsBuilder, ListContainersOptionsBuilder};
use futures_util::stream::BoxStream;

use crate::model::{Action, Container, ContainerEvent, Runtime};

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
        filters.insert(
            "event".into(),
            vec!["start".into(), "die".into(), "health_status".into()],
        );

        let options = EventsOptionsBuilder::default().filters(&filters).build();
        Box::pin(self.client.events(Some(options)))
    }

    pub fn handle_container_event(event: EventMessage) -> ContainerEvent {
        let action = event.action.unwrap_or_default();
        let id = event.actor.unwrap_or_default().id.unwrap_or_default();

        ContainerEvent {
            id,
            action: Action::from(action),
        }
    }
}
