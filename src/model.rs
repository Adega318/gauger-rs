use std::collections::VecDeque;
use std::fmt::Display;

use bollard::plugin::{ContainerSummary, ContainerSummaryStateEnum};

const HISTORY_SIZE: usize = 100;

#[derive(Debug, Copy, Clone)]
pub enum Runtime {
    Docker,
    Podman,
}

impl Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Runtime::Docker => "docker",
                Runtime::Podman => "podman",
            }
        )
    }
}

#[derive(Debug)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub runtime: Runtime,
}

impl Container {
    pub fn from_summary(container: ContainerSummary, runtime: &Runtime) -> Self {
        Container {
            id: container.id.unwrap_or_default(),
            name: container
                .names
                .unwrap_or_default()
                .into_iter()
                .next()
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_owned(),
            image: container.image.unwrap_or_default(),
            state: ContainerState::from(container.state),
            runtime: *runtime,
        }
    }
}

pub struct ContainerStatsHistory {
    pub hist: VecDeque<ContainerStats>,
}

impl ContainerStatsHistory {
    pub fn new() -> Self {
        Self {
            hist: VecDeque::with_capacity(HISTORY_SIZE),
        }
    }

    pub fn push(&mut self, stats: ContainerStats) {
        if self.hist.len() == HISTORY_SIZE {
            self.hist.pop_front();
        }
        self.hist.push_back(stats);
    }

    pub fn latest(&self) -> Option<&ContainerStats> {
        self.hist.back()
    }
}

impl Default for ContainerStatsHistory {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ContainerStats {
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: Option<u64>,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
}

#[derive(Debug)]
pub enum ContainerState {
    Running,
    Paused,
    Exited,
    Created,
    Restarting,
    Removing,
    Dead,
    Stopping,
    Empty,
    Unknown,
}

impl From<Option<ContainerSummaryStateEnum>> for ContainerState {
    fn from(option: Option<ContainerSummaryStateEnum>) -> Self {
        match option {
            Some(ContainerSummaryStateEnum::CREATED) => Self::Created,
            Some(ContainerSummaryStateEnum::RUNNING) => Self::Running,
            Some(ContainerSummaryStateEnum::PAUSED) => Self::Paused,
            Some(ContainerSummaryStateEnum::RESTARTING) => Self::Restarting,
            Some(ContainerSummaryStateEnum::EXITED) => Self::Exited,
            Some(ContainerSummaryStateEnum::REMOVING) => Self::Removing,
            Some(ContainerSummaryStateEnum::DEAD) => Self::Dead,
            Some(ContainerSummaryStateEnum::STOPPING) => Self::Stopping,
            Some(ContainerSummaryStateEnum::EMPTY) => Self::Empty,
            None => Self::Unknown,
        }
    }
}

impl ContainerState {
    pub fn from_docker(state: &str) -> Self {
        match state.to_ascii_lowercase().as_str() {
            "running" => Self::Running,
            "paused" => Self::Paused,
            "exited" | "dead" => Self::Exited,
            "created" => Self::Created,
            _ => Self::Unknown,
        }
    }
}
