use std::fmt::Display;

use bollard::plugin::{
    ContainerSummary, ContainerSummaryHealthStatusEnum, ContainerSummaryStateEnum, HealthStatusEnum,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Runtime {
    Docker,
    Podman,
}

impl Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Health {
    Starting,
    Healthy,
    Unhealthy,
    Empty,
    #[default]
    Unknown,
}

impl From<Option<ContainerSummaryHealthStatusEnum>> for Health {
    fn from(health: Option<ContainerSummaryHealthStatusEnum>) -> Self {
        match health {
            Some(ContainerSummaryHealthStatusEnum::STARTING) => Self::Starting,
            Some(ContainerSummaryHealthStatusEnum::HEALTHY) => Self::Healthy,
            Some(ContainerSummaryHealthStatusEnum::UNHEALTHY) => Self::Unhealthy,
            Some(ContainerSummaryHealthStatusEnum::EMPTY) => Self::Empty,
            Some(ContainerSummaryHealthStatusEnum::NONE) | None => Self::Unknown,
        }
    }
}

impl From<Option<HealthStatusEnum>> for Health {
    fn from(health: Option<HealthStatusEnum>) -> Self {
        match health {
            Some(HealthStatusEnum::STARTING) => Self::Starting,
            Some(HealthStatusEnum::HEALTHY) => Self::Healthy,
            Some(HealthStatusEnum::UNHEALTHY) => Self::Unhealthy,
            Some(HealthStatusEnum::EMPTY) => Self::Empty,
            Some(HealthStatusEnum::NONE) | None => Self::Unknown,
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

impl Display for Health {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            Self::Healthy => "healthy",
            Self::Unhealthy => "unhealthy",
            Self::Starting => "starting",
            Self::Empty => "-",
            Self::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
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

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Exited => "exited",
            Self::Created => "created",
            Self::Restarting => "restarting",
            Self::Removing => "removing",
            Self::Dead => "dead",
            Self::Stopping => "stopping",
            Self::Empty => "empty",
            Self::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Create,
    Start,
    Stop,
    Die,
    Pause,
    Unpause,
    Restart,
    Remove,
    Rename,
    Oom,
    Health(Health),
    Unknown(String),
}

impl From<&str> for Action {
    fn from(action: &str) -> Self {
        match action {
            "create" => Self::Create,
            "start" => Self::Start,
            "stop" | "kill" => Self::Stop,
            "die" | "died" | "exited" => Self::Die,
            "pause" => Self::Pause,
            "unpause" => Self::Unpause,
            "restart" => Self::Restart,
            "destroy" | "remove" => Self::Remove,
            "rename" => Self::Rename,
            "oom" => Self::Oom,
            "health_status" => Self::Health(Health::Unknown),
            action if let Some(status) = action.strip_prefix("health_status: ") => {
                Self::Health(Health::from(status))
            }
            other => Self::Unknown(other.to_owned()),
        }
    }
}

impl From<String> for Action {
    fn from(action: String) -> Self {
        Self::from(action.as_str())
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(action) => f.pad(&format!("unknown: {action}")),
            other => f.pad(match other {
                Self::Create => "create",
                Self::Start => "start",
                Self::Stop => "stop",
                Self::Die => "die",
                Self::Pause => "pause",
                Self::Unpause => "unpause",
                Self::Restart => "restart",
                Self::Remove => "remove",
                Self::Rename => "rename",
                Self::Oom => "oom",
                Self::Health(_) => "health",
                Self::Unknown(_) => unreachable!(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerEvent {
    pub runtime: Runtime,
    pub id: String,
    pub name: Option<String>,
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_docker_lifecycle_actions() {
        assert_eq!(Action::from("create"), Action::Create);
        assert_eq!(Action::from("start"), Action::Start);
        assert_eq!(Action::from("stop"), Action::Stop);
        assert_eq!(Action::from("die"), Action::Die);
        assert_eq!(Action::from("pause"), Action::Pause);
        assert_eq!(Action::from("unpause"), Action::Unpause);
        assert_eq!(Action::from("restart"), Action::Restart);
        assert_eq!(Action::from("destroy"), Action::Remove);
        assert_eq!(Action::from("rename"), Action::Rename);
        assert_eq!(Action::from("oom"), Action::Oom);
    }

    #[test]
    fn maps_podman_specific_actions() {
        assert_eq!(Action::from("died"), Action::Die);
        assert_eq!(Action::from("exited"), Action::Die);
        assert_eq!(Action::from("remove"), Action::Remove);
        assert_eq!(Action::from("kill"), Action::Stop);
    }

    #[test]
    fn maps_health_actions() {
        assert_eq!(
            Action::from("health_status: healthy"),
            Action::Health(Health::Healthy)
        );
        assert_eq!(
            Action::from("health_status: unhealthy"),
            Action::Health(Health::Unhealthy)
        );
        assert_eq!(
            Action::from("health_status"),
            Action::Health(Health::Unknown)
        );
    }

    #[test]
    fn display_impls_honor_padding() {
        assert_eq!(format!("{:<7}", Runtime::Docker), "docker ");
        assert_eq!(format!("{:<10}", State::Running), "running   ");
        assert_eq!(format!("{:<9}", Health::Healthy), "healthy  ");
        assert_eq!(format!("{:<8}", Action::Start), "start   ");
    }

    #[test]
    fn preserves_unknown_actions() {
        assert_eq!(
            Action::from("exec_start: /bin/sh"),
            Action::Unknown("exec_start: /bin/sh".to_owned())
        );
    }
}
