use bollard::plugin::{
    ContainerSummary, ContainerSummaryHealthStatusEnum, ContainerSummaryStateEnum, HealthStatusEnum,
};

use super::{Container, Health, Runtime, State};

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

#[cfg(test)]
mod tests {
    use crate::model::Action;

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
