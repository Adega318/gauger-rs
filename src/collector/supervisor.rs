use std::time::Duration;

use tokio::{
    sync::{broadcast, mpsc},
    time::sleep,
};

use crate::runtime::RuntimeClient;

use super::{command::CollectorCommand, event::CollectorEvent, runtime::run_once};

pub(crate) const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub(super) enum LoopOutcome {
    ChannelClosed,
    Failed(String),
}

pub(super) async fn watch_runtime(
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
