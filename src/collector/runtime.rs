use std::{fmt::Display, future::Future};

use futures_util::{Stream, StreamExt};
use tokio::sync::{broadcast, mpsc};

use bollard::plugin::EventMessage;

use crate::{
    model::{Container, Runtime},
    runtime::RuntimeClient,
};

use super::{
    command::CollectorCommand, event::CollectorEvent, forwarding::forward_event,
    supervisor::LoopOutcome,
};

const MAX_BUFFERED_EVENTS: usize = 4096;

pub(super) async fn run_once(
    client: &RuntimeClient,
    tx: &mpsc::Sender<CollectorEvent>,
    commands: &mut broadcast::Receiver<CollectorCommand>,
    backoff: &mut std::time::Duration,
) -> LoopOutcome {
    let runtime = client.runtime();
    let mut events = client.get_event_stream();

    let snapshot = client.list_containers();

    let (containers, buffered) = match collect_snapshot(snapshot, &mut events).await {
        Ok(result) => result,
        Err(reason) => return LoopOutcome::Failed(reason),
    };

    if !send_snapshot(tx, runtime, containers).await {
        return LoopOutcome::ChannelClosed;
    }

    *backoff = super::supervisor::MIN_BACKOFF;

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
                        return LoopOutcome::Failed(
                            format!("event stream failed: {err}")
                        );
                    }
                    None => {
                        return LoopOutcome::Failed(
                            "event stream ended unexpectedly".to_string()
                        );
                    }
                }
            }

            command = commands.recv() => {
                match command {
                    Ok(command) if command.applies_to(runtime) => {
                        match client.list_containers().await {
                            Ok(containers) => {
                                if !send_snapshot(tx, runtime, containers).await {
                                    return LoopOutcome::ChannelClosed;
                                }
                            }
                            Err(err) => {
                                return LoopOutcome::Failed(err.to_string());
                            }
                        }
                    }

                    Ok(_) => {}

                    Err(broadcast::error::RecvError::Lagged(_)) => {}

                    Err(broadcast::error::RecvError::Closed) => {
                        return LoopOutcome::ChannelClosed;
                    }
                }
            }
        }
    }
}

async fn collect_snapshot<F, S, E1, E2>(
    snapshot: F,
    events: &mut S,
) -> Result<(Vec<Container>, Vec<EventMessage>), String>
where
    F: Future<Output = Result<Vec<Container>, E1>>,
    S: Stream<Item = Result<EventMessage, E2>> + Unpin,
    E1: Display,
    E2: Display,
{
    tokio::pin!(snapshot);

    let mut buffered = Vec::new();

    loop {
        tokio::select! {
            result = &mut snapshot => {
                return result
                    .map(|containers| (containers, buffered))
                    .map_err(|err| err.to_string());
            }

            event = events.next() => {
                match event {
                    Some(Ok(event)) => {
                        if buffered.len() >= MAX_BUFFERED_EVENTS {
                            return Err(
                                "too many events while collecting the container snapshot"
                                    .to_string()
                            );
                        }

                        buffered.push(event);
                    }
                    Some(Err(err)) => {
                        return Err(format!("event stream failed: {err}"));
                    }
                    None => {
                        return Err(
                            "event stream ended unexpectedly".to_string()
                        );
                    }
                }
            }
        }
    }
}

async fn send_snapshot(
    tx: &mpsc::Sender<CollectorEvent>,
    runtime: Runtime,
    containers: Vec<Container>,
) -> bool {
    tx.send(CollectorEvent::Snapshot {
        runtime,
        containers,
    })
    .await
    .is_ok()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::Poll;

    use futures_util::stream;

    use super::*;
    use crate::model::{Health, State};

    fn container(id: &str) -> Container {
        Container {
            id: id.to_owned(),
            name: format!("name-{id}"),
            image: "image:latest".to_owned(),
            runtime: Runtime::Docker,
            state: State::Running,
            health: Health::Healthy,
        }
    }

    fn event(id: &str) -> EventMessage {
        EventMessage {
            typ: Some(bollard::plugin::EventMessageTypeEnum::CONTAINER),
            action: Some("start".to_owned()),
            actor: Some(bollard::plugin::EventActor {
                id: Some(id.to_owned()),
                attributes: None,
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn collect_snapshot_returns_containers_when_snapshot_wins() {
        let snapshot = async { Ok::<_, String>(vec![container("c1"), container("c2")]) };
        let mut events = stream::pending::<Result<EventMessage, String>>();

        let (containers, buffered) = collect_snapshot(snapshot, &mut events).await.unwrap();

        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].id, "c1");
        assert!(buffered.is_empty());
    }

    #[tokio::test]
    async fn collect_snapshot_buffers_events_until_snapshot_resolves() {
        let delivered = Arc::new(AtomicUsize::new(0));

        let mut queued = (0..3)
            .map(|index| Ok::<_, String>(event(&format!("e{index}"))))
            .collect::<VecDeque<_>>();
        let mut events = stream::poll_fn({
            let delivered = Arc::clone(&delivered);
            move |_| match queued.pop_front() {
                Some(event) => {
                    delivered.fetch_add(1, Ordering::SeqCst);
                    Poll::Ready(Some(event))
                }
                None => Poll::Pending,
            }
        });

        let snapshot = {
            let delivered = Arc::clone(&delivered);
            async move {
                while delivered.load(Ordering::SeqCst) < 3 {
                    tokio::task::yield_now().await;
                }
                Ok::<_, String>(vec![container("snapshot")])
            }
        };

        let (containers, buffered) = collect_snapshot(snapshot, &mut events).await.unwrap();

        assert_eq!(containers[0].id, "snapshot");
        assert_eq!(buffered.len(), 3);
        let ids: Vec<_> = buffered
            .iter()
            .map(|event| event.actor.as_ref().unwrap().id.clone().unwrap())
            .collect();
        assert_eq!(ids, vec!["e0", "e1", "e2"]);
    }

    #[tokio::test]
    async fn collect_snapshot_reports_snapshot_failure() {
        let snapshot = async { Err::<Vec<Container>, _>("snapshot boom".to_owned()) };
        let mut events = stream::pending::<Result<EventMessage, String>>();

        let error = collect_snapshot(snapshot, &mut events).await.unwrap_err();

        assert_eq!(error, "snapshot boom");
    }

    #[tokio::test]
    async fn collect_snapshot_reports_event_stream_failure() {
        let snapshot = std::future::pending::<Result<Vec<Container>, String>>();
        let mut events = stream::iter([Err("stream boom".to_owned())]);

        let error = collect_snapshot(snapshot, &mut events).await.unwrap_err();

        assert_eq!(error, "event stream failed: stream boom");
    }

    #[tokio::test]
    async fn collect_snapshot_reports_early_event_stream_end() {
        let snapshot = std::future::pending::<Result<Vec<Container>, String>>();
        let mut events = stream::iter(Vec::<Result<EventMessage, String>>::new());

        let error = collect_snapshot(snapshot, &mut events).await.unwrap_err();

        assert_eq!(error, "event stream ended unexpectedly");
    }

    #[tokio::test]
    async fn collect_snapshot_rejects_event_overflow() {
        let snapshot = std::future::pending::<Result<Vec<Container>, String>>();
        let mut events = stream::iter(
            (0..=MAX_BUFFERED_EVENTS).map(|index| Ok::<_, String>(event(&format!("e{index}")))),
        );

        let error = collect_snapshot(snapshot, &mut events).await.unwrap_err();

        assert_eq!(
            error,
            "too many events while collecting the container snapshot"
        );
    }

    #[tokio::test]
    async fn send_snapshot_delivers_snapshot() {
        let (tx, mut rx) = mpsc::channel(1);

        assert!(send_snapshot(&tx, Runtime::Docker, vec![container("c1")]).await);

        match rx.recv().await.unwrap() {
            CollectorEvent::Snapshot {
                runtime,
                containers,
            } => {
                assert_eq!(runtime, Runtime::Docker);
                assert_eq!(containers.len(), 1);
                assert_eq!(containers[0].id, "c1");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_snapshot_reports_closed_channel() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);

        assert!(!send_snapshot(&tx, Runtime::Podman, Vec::new()).await);
    }
}
