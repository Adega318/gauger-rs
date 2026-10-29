use std::time::Duration;

use bollard::plugin::EventMessage;
use futures_util::StreamExt;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::model::{Action, Container, ContainerEvent, Health, Runtime};
use crate::runtime::RuntimeClient;

const CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CAPACITY: usize = 16;
const MAX_BUFFERED_EVENTS: usize = 4096;
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const HEALTH_INSPECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorCommand {
    Refresh(Runtime),
    RefreshAll,
}

impl CollectorCommand {
    fn applies_to(&self, runtime: Runtime) -> bool {
        match self {
            Self::Refresh(target) => *target == runtime,
            Self::RefreshAll => true,
        }
    }
}

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
        let (commands, _) = broadcast::channel(COMMAND_CAPACITY);

        let tasks = self
            .clients
            .into_iter()
            .map(|client| {
                let tx = tx.clone();
                let commands = commands.subscribe();
                tokio::spawn(watch_runtime(client, tx, commands))
            })
            .collect();

        CollectorHandle {
            events,
            commands,
            tasks,
        }
    }
}

pub struct CollectorHandle {
    events: mpsc::Receiver<CollectorEvent>,
    commands: broadcast::Sender<CollectorCommand>,
    tasks: Vec<JoinHandle<()>>,
}

impl CollectorHandle {
    pub async fn recv(&mut self) -> Option<CollectorEvent> {
        self.events.recv().await
    }

    pub fn refresh(&self, runtime: Runtime) {
        let _ = self.commands.send(CollectorCommand::Refresh(runtime));
    }

    pub fn refresh_all(&self) {
        let _ = self.commands.send(CollectorCommand::RefreshAll);
    }
}

impl Drop for CollectorHandle {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn watch_runtime(
    client: RuntimeClient,
    tx: mpsc::Sender<CollectorEvent>,
    mut commands: broadcast::Receiver<CollectorCommand>,
) {
    let runtime = client.runtime();
    let mut backoff = MIN_BACKOFF;

    loop {
        match run_once(&client, &tx, &mut commands, &mut backoff).await {
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
    commands: &mut broadcast::Receiver<CollectorCommand>,
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

    loop {
        tokio::select! {
            result = events.next() => {
                match result {
                    Some(Ok(event)) => {
                        if !forward_event(client, tx, event).await {
                            return LoopOutcome::ChannelClosed;
                        }
                    }
                    Some(Err(err)) => {
                        return LoopOutcome::Failed(format!("event stream failed: {err}"));
                    }
                    None => {
                        return LoopOutcome::Failed("event stream ended unexpectedly".to_string());
                    }
                }
            }
            command = commands.recv() => {
                match command {
                    Ok(command) if command.applies_to(runtime) => {
                        match client.list_containers().await {
                            Ok(containers) => {
                                if tx
                                    .send(CollectorEvent::Snapshot { runtime, containers })
                                    .await
                                    .is_err()
                                {
                                    return LoopOutcome::ChannelClosed;
                                }
                            }
                            Err(err) => return LoopOutcome::Failed(err.to_string()),
                        }
                    }
                    // Commands for other runtimes are not ours to handle.
                    Ok(_) => {}
                    // Dropping stale refresh requests is safe; a periodic resync
                    // or the next manual refresh will catch up.
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => {
                        return LoopOutcome::ChannelClosed;
                    }
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_command_targets_one_runtime() {
        assert!(CollectorCommand::Refresh(Runtime::Docker).applies_to(Runtime::Docker));
        assert!(!CollectorCommand::Refresh(Runtime::Docker).applies_to(Runtime::Podman));
    }

    #[test]
    fn refresh_all_applies_to_every_runtime() {
        assert!(CollectorCommand::RefreshAll.applies_to(Runtime::Docker));
        assert!(CollectorCommand::RefreshAll.applies_to(Runtime::Podman));
    }
}
