use std::fmt::Display;

use bollard::plugin::{
    ContainerSummary, ContainerSummaryHealthStatusEnum, ContainerSummaryStateEnum,
};

#[derive(Debug, Clone, Copy)]
pub enum Runtime {
    Docker,
    Podman,
}

impl Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        })
    }
}

#[derive(Debug, Default)]
pub enum Health {
    Starting,
    Healthy,
    Unhealthy,
    #[default]
    Unknown,
}

impl From<Option<ContainerSummaryHealthStatusEnum>> for Health {
    fn from(health: Option<ContainerSummaryHealthStatusEnum>) -> Self {
        match health {
            Some(ContainerSummaryHealthStatusEnum::STARTING) => Self::Starting,
            Some(ContainerSummaryHealthStatusEnum::HEALTHY) => Self::Healthy,
            Some(ContainerSummaryHealthStatusEnum::UNHEALTHY) => Self::Unhealthy,
            Some(ContainerSummaryHealthStatusEnum::NONE)
            | Some(ContainerSummaryHealthStatusEnum::EMPTY)
            | None => Self::Unknown,
        }
    }
}

impl From<&str> for Health {
    fn from(status: &str) -> Self {
        match status {
            "starting" => Self::Starting,
            "healthy" => Self::Healthy,
            "unhealthy" => Self::Unhealthy,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Default)]
pub enum State {
    Running,
    Paused,
    Exited,
    Created,
    Restarting,
    Removing,
    Dead,
    Stopping,
    Empty,
    #[default]
    Unknown,
}

impl From<Option<ContainerSummaryStateEnum>> for State {
    fn from(option: Option<ContainerSummaryStateEnum>) -> Self {
        match option {
            Some(state) => match state {
                ContainerSummaryStateEnum::CREATED => Self::Created,
                ContainerSummaryStateEnum::RUNNING => Self::Running,
                ContainerSummaryStateEnum::PAUSED => Self::Paused,
                ContainerSummaryStateEnum::RESTARTING => Self::Restarting,
                ContainerSummaryStateEnum::EXITED => Self::Exited,
                ContainerSummaryStateEnum::REMOVING => Self::Removing,
                ContainerSummaryStateEnum::DEAD => Self::Dead,
                ContainerSummaryStateEnum::STOPPING => Self::Stopping,
                ContainerSummaryStateEnum::EMPTY => Self::Empty,
            },
            None => Self::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub runtime: Runtime,
    pub state: State,
    pub health: Health,
}

impl Container {
    pub fn from_summary(container: ContainerSummary, runtime: Runtime) -> Self {
        Self {
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
            runtime,
            state: State::from(container.state),
            health: Health::from(container.health.and_then(|health| health.status)),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Start,
    Die,
    Health(Health),
    Unknown(String),
}

impl From<String> for Action {
    fn from(action: String) -> Self {
        match action.as_str() {
            "start" => Self::Start,
            "die" => Self::Die,
            action if let Some(status) = action.strip_prefix("health_status: ") => {
                Self::Health(Health::from(status))
            }
            _ => Self::Unknown(action),
        }
    }
}

#[derive(Debug)]
pub struct ContainerEvent {
    pub id: String,
    pub action: Action,
}
