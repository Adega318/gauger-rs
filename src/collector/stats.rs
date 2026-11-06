use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use futures_util::stream::{self, StreamExt};
use tokio::{
    sync::mpsc,
    time::{MissedTickBehavior, interval, timeout},
};

use crate::{
    model::{ResourceUsage, State, StatsCounters},
    runtime::RuntimeClient,
};

use super::event::CollectorEvent;

const STATS_INTERVAL: Duration = Duration::from_secs(1);
const STATS_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CONCURRENT_STATS: usize = 8;

/// Periodically sample running containers and forward batched usage updates.
pub(super) async fn watch_stats(client: RuntimeClient, tx: mpsc::Sender<CollectorEvent>) {
    let runtime = client.runtime();
    let mut tick = interval(STATS_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut previous = HashMap::new();

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let samples = collect(&client, &mut previous).await;

                if samples.is_empty() {
                    continue;
                }

                if tx
                    .send(CollectorEvent::Stats { runtime, samples })
                    .await
                    .is_err()
                {
                    return;
                }
            }

            _ = tx.closed() => return,
        }
    }
}

async fn collect(
    client: &RuntimeClient,
    previous: &mut HashMap<String, StatsCounters>,
) -> Vec<(String, ResourceUsage)> {
    let containers = match timeout(STATS_TIMEOUT, client.list_containers()).await {
        Ok(Ok(containers)) => containers,
        _ => return Vec::new(),
    };

    let running: Vec<String> = containers
        .into_iter()
        .filter(|container| container.state == State::Running)
        .map(|container| container.id)
        .collect();

    let running_ids: HashSet<String> = running.iter().cloned().collect();
    previous.retain(|id, _| running_ids.contains(id));

    let results = stream::iter(running.into_iter().map(|id| async move {
        let stats = timeout(STATS_TIMEOUT, client.stats_once(&id)).await;
        (id, stats)
    }))
    .buffer_unordered(MAX_CONCURRENT_STATS)
    .collect::<Vec<_>>()
    .await;

    let mut samples = Vec::with_capacity(results.len());

    for (id, result) in results {
        let Ok(Ok(stats)) = result else {
            continue;
        };

        let now = Instant::now();
        let (usage, counters) = ResourceUsage::from_stats(&stats, previous.get(&id), now);

        previous.insert(id.clone(), counters);
        samples.push((id, usage));
    }

    samples
}
