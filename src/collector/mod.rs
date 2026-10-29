use std::time::Duration;

use bollard::plugin::EventMessage;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::model::{Action, Container, ContainerEvent, Health, Runtime};
use crate::runtime::RuntimeClient;

const CHANNEL_CAPACITY: usize = 1024;
const MAX_BUFFERED_EVENTS: usize = 4096;
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const HEALTH_INSPECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum CollectorEvent {
    Snapshot {
        runtime: Runtime,
        containers: Vec<Container>,
    },
    Container(ContainerEvent),
    RuntimeUp {
        runtime: Runtime,
    },
    RuntimeDown {
        runtime: Runtime,
        reason: String,
    },
}

pub struct Collector {
    clients: Vec<RuntimeClient>,
}

impl Collector {
    pub fn new(clients: Vec<RuntimeClient>) -> Self {
        Self { clients }
    }

    pub fn spawn(self) -> CollectorHandle {
        let (tx, events) = mpsc::channel(CHANNEL_CAPACITY);

        let tasks = self
            .clients
            .into_iter()
            .map(|client| {
                let tx = tx.clone();
                tokio::spawn(watch_runtime(client, tx))
            })
            .collect();

        CollectorHandle { events, tasks }
    }
}

pub struct CollectorHandle {
    events: mpsc::Receiver<CollectorEvent>,
    tasks: Vec<JoinHandle<()>>,
}

impl CollectorHandle {
    pub async fn recv(&mut self) -> Option<CollectorEvent> {
        self.events.recv().await
    }
}

impl Drop for CollectorHandle {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn watch_runtime(client: RuntimeClient, tx: mpsc::Sender<CollectorEvent>) {
    let runtime = client.runtime();
    let mut backoff = MIN_BACKOFF;

    loop {
        match run_once(&client, &tx, &mut backoff).await {
            LoopOutcome::ChannelClosed => return,
            LoopOutcome::Failed(reason) => {
                if tx
                    .send(CollectorEvent::RuntimeDown { runtime, reason })
                    .await
                    .is_err()
                {
                    return;
                }

                sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

enum LoopOutcome {
    ChannelClosed,
    Failed(String),
}

async fn run_once(
    client: &RuntimeClient,
    tx: &mpsc::Sender<CollectorEvent>,
    backoff: &mut Duration,
) -> LoopOutcome {
    let runtime = client.runtime();

    let mut events = client.get_event_stream();
    let snapshot = client.list_containers();
    tokio::pin!(snapshot);

    let mut buffered = Vec::new();

    let containers = loop {
        tokio::select! {
            result = &mut snapshot => {
                match result {
                    Ok(containers) => break containers,
                    Err(err) => return LoopOutcome::Failed(err.to_string()),
                }
            }
            event = events.next() => {
                match event {
                    Some(Ok(event)) => {
                        if buffered.len() >= MAX_BUFFERED_EVENTS {
                            return LoopOutcome::Failed(
                                "too many events while collecting the container snapshot".to_string(),
                            );
                        }
                        buffered.push(event);
                    }
                    Some(Err(err)) => {
                        return LoopOutcome::Failed(format!("event stream failed: {err}"));
                    }
                    None => {
                        return LoopOutcome::Failed("event stream ended unexpectedly".to_string());
                    }
                }
            }
        }
    };

    if tx
        .send(CollectorEvent::Snapshot {
            runtime,
            containers,
        })
        .await
        .is_err()
    {
        return LoopOutcome::ChannelClosed;
    }

    *backoff = MIN_BACKOFF;

    if tx
        .send(CollectorEvent::RuntimeUp { runtime })
        .await
        .is_err()
    {
        return LoopOutcome::ChannelClosed;
    }

    for event in buffered {
        if !forward_event(client, tx, event).await {
            return LoopOutcome::ChannelClosed;
        }
    }

    while let Some(result) = events.next().await {
        match result {
            Ok(event) => {
                if !forward_event(client, tx, event).await {
                    return LoopOutcome::ChannelClosed;
                }
            }
            Err(err) => return LoopOutcome::Failed(format!("event stream failed: {err}")),
        }
    }

    LoopOutcome::Failed("event stream ended unexpectedly".to_string())
}

async fn forward_event(
    client: &RuntimeClient,
    tx: &mpsc::Sender<CollectorEvent>,
    event: EventMessage,
) -> bool {
    let Some(mut event) = client.normalize_event(event) else {
        return true;
    };

    if matches!(&event.action, Action::Health(Health::Unknown))
        && let Ok(Ok(health)) =
            timeout(HEALTH_INSPECT_TIMEOUT, client.inspect_health(&event.id)).await
    {
        event.action = Action::Health(health);
    }

    tx.send(CollectorEvent::Container(event)).await.is_ok()
}
