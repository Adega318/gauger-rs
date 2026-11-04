use std::fmt::Display;

use super::{Action, Container, ContainerEvent, Health, Runtime, State};

impl Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        })
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

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
