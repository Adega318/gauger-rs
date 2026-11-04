use std::fmt::{Display, Formatter};

use bollard::plugin::{
    ContainerSummary, ContainerSummaryHealthStatusEnum, ContainerSummaryStateEnum, HealthStatusEnum,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Runtime {
    Docker,
    Podman,
}

impl Runtime {
    pub const fn all() -> &'static [Self] {
        &[Self::Docker, Self::Podman]
    }
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
    #[default]
    Empty,
}

impl From<Option<ContainerSummaryHealthStatusEnum>> for Health {
    fn from(health: Option<ContainerSummaryHealthStatusEnum>) -> Self {
        match health {
            Some(ContainerSummaryHealthStatusEnum::STARTING) => Self::Starting,
            Some(ContainerSummaryHealthStatusEnum::HEALTHY) => Self::Healthy,
            Some(ContainerSummaryHealthStatusEnum::UNHEALTHY) => Self::Unhealthy,
            Some(ContainerSummaryHealthStatusEnum::EMPTY)
            | Some(ContainerSummaryHealthStatusEnum::NONE)
            | None => Self::Empty,
        }
    }
}

impl From<Option<HealthStatusEnum>> for Health {
    fn from(health: Option<HealthStatusEnum>) -> Self {
        match health {
            Some(HealthStatusEnum::STARTING) => Self::Starting,
            Some(HealthStatusEnum::HEALTHY) => Self::Healthy,
            Some(HealthStatusEnum::UNHEALTHY) => Self::Unhealthy,
            Some(HealthStatusEnum::EMPTY) | Some(HealthStatusEnum::NONE) | None => Self::Empty,
        }
    }
}

impl From<&str> for Health {
    fn from(status: &str) -> Self {
        match status {
            "starting" => Self::Starting,
            "healthy" => Self::Healthy,
            "unhealthy" => Self::Unhealthy,
            _ => Self::Empty,
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
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Stopping,
    #[default]
    Empty,
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
            None => Self::Empty,
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
    pub compose_project: Option<String>,
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
            compose_project: container
                .labels
                .unwrap_or_default()
                .get("com.docker.compose.project")
                .cloned(),
        }
    }
}

impl Display for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) [{}] - {} ({})",
            self.name, self.id, self.runtime, self.state, self.health,
        )?;

        if let Some(project) = self.compose_project.clone() {
            write!(f, " - compose: {}", project)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Create,
    Start,
    Stop,
    Die,
    Kill,
    Restart,
    Pause,
    Unpause,
    Remove,
    Oom,
    Rename,
    Update,
    Health(Health),
    Unknown(String),
}

impl From<&str> for Action {
    fn from(value: &str) -> Self {
        match value {
            "create" => Self::Create,
            "start" => Self::Start,
            "stop" => Self::Stop,
            "die" | "died" | "exited" => Self::Die,
            "kill" => Self::Kill,
            "restart" => Self::Restart,
            "pause" => Self::Pause,
            "unpause" => Self::Unpause,
            "remove" | "destroy" => Self::Remove,
            "oom" => Self::Oom,
            "health_status" => Self::Health(Health::Empty),
            action if let Some(status) = action.strip_prefix("health_status: ") => {
                Self::Health(Health::from(status))
            }
            "rename" => Self::Rename,
            "update" => Self::Update,
            action => Self::Unknown(action.to_owned()),
        }
    }
}

impl From<String> for Action {
    fn from(action: String) -> Self {
        Self::from(action.as_str())
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Create => "create",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Die => "die",
            Self::Kill => "kill",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
            Self::Remove => "remove",
            Self::Oom => "oom",
            Self::Rename => "rename",
            Self::Update => "update",
            Self::Health(health) => return write!(f, "health({health})"),
            Self::Unknown(action) => return write!(f, "unknown: {action}"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerEvent {
    pub runtime: Runtime,
    pub id: String,
    pub name: Option<String>,
    pub action: Action,
}

impl Display for ContainerEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {} ({}) - {}",
            self.runtime,
            self.name.as_deref().unwrap_or(&self.id),
            self.id,
            self.action,
        )
    }
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
        assert_eq!(Action::from("rename"), Action::Rename);
        assert_eq!(Action::from("oom"), Action::Oom);
    }

    #[test]
    fn maps_podman_specific_actions() {
        assert_eq!(Action::from("died"), Action::Die);
        assert_eq!(Action::from("exited"), Action::Die);
        assert_eq!(Action::from("kill"), Action::Kill);
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
        assert_eq!(Action::from("health_status"), Action::Health(Health::Empty));
    }

    #[test]
    fn preserves_unknown_actions() {
        assert_eq!(
            Action::from("exec_start: /bin/sh"),
            Action::Unknown("exec_start: /bin/sh".to_owned())
        );
    }
}
