use std::time::Duration;

use bollard::plugin::EventMessage;
use tokio::{sync::mpsc, time::timeout};

use crate::{
    model::{Action, ContainerEvent, Health},
    runtime::RuntimeClient,
};

use super::event::CollectorEvent;

const HEALTH_INSPECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn forward_event(
    client: &RuntimeClient,
    tx: &mpsc::Sender<CollectorEvent>,
    event: EventMessage,
) -> bool {
    let Some(mut event) = client.normalize_event(event) else {
        return true;
    };

    enrich_health(client, &mut event).await;

    tx.send(CollectorEvent::Container(event)).await.is_ok()
}

async fn enrich_health(client: &RuntimeClient, event: &mut ContainerEvent) {
    if !matches!(&event.action, Action::Health(Health::Unknown)) {
        return;
    }

    if let Ok(Ok(health)) = timeout(HEALTH_INSPECT_TIMEOUT, client.inspect_health(&event.id)).await
    {
        event.action = Action::Health(health);
    }
}
