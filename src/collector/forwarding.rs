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

    let probe = matches!(event.action, Action::Start | Action::Restart)
        .then(|| (event.runtime, event.id.clone()));

    if tx.send(CollectorEvent::Container(event)).await.is_err() {
        return false;
    }

    if let Some((runtime, id)) = probe
        && let Ok(Ok(health)) = timeout(HEALTH_INSPECT_TIMEOUT, client.inspect_health(&id)).await
    {
        let follow_up = ContainerEvent {
            runtime,
            id,
            name: None,
            action: Action::Health(health),
        };
        return tx.send(CollectorEvent::Container(follow_up)).await.is_ok();
    }

    true
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
