mod command;
mod event;
mod forwarding;
mod runtime;
mod supervisor;

pub use command::CollectorCommand;
pub use event::CollectorEvent;

use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

use crate::runtime::RuntimeClient;

const CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CAPACITY: usize = 16;

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

                tokio::spawn(supervisor::watch_runtime(client, tx, commands))
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

    pub fn refresh(&self, runtime: crate::model::Runtime) {
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
