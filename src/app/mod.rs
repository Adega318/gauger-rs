use std::collections::HashMap;

use crate::collector::CollectorEvent;
use crate::model::{Action, Container, Health, Runtime, State};

use self::error::AppError;

pub mod error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Up,
    Down,
    Unavailable,
}

pub struct App {
    runtimes: HashMap<Runtime, RuntimeStatus>,
    containers: HashMap<Runtime, HashMap<String, Container>>,
}

impl App {
    pub fn new(runtimes: Vec<Runtime>) -> Self {
        Self {
            runtimes: runtimes
                .into_iter()
                .map(|runtime| (runtime, RuntimeStatus::Down))
                .collect(),
            containers: HashMap::new(),
        }
    }

    pub fn apply_event(&mut self, collector_event: CollectorEvent) -> Result<(), AppError> {
        match collector_event {
            CollectorEvent::Snapshot {
                runtime,
                containers,
            } => {
                let containers = containers
                    .into_iter()
                    .map(|container| (container.id.clone(), container))
                    .collect();

                self.containers.insert(runtime, containers);
            }
            CollectorEvent::Container(container_event) => {
                let runtime = &container_event.runtime;
                let container_id = container_event.id.as_str();

                match container_event.action {
                    Action::Create => {
                        todo!("set refresh to get the snapshot");
                    }
                    Action::Start => {
                        self.set_state(runtime, container_id, State::Running)?;
                        self.set_health(runtime, container_id, Health::Starting)?;
                    }
                    Action::Stop => {
                        self.set_state(runtime, container_id, State::Stopping)?;
                    }
                    Action::Die => {
                        self.set_state(runtime, container_id, State::Exited)?;
                        self.set_health(runtime, container_id, Health::Empty)?;
                    }
                    Action::Pause => {
                        self.set_state(runtime, container_id, State::Paused)?;
                    }
                    Action::Unpause => {
                        self.set_state(runtime, container_id, State::Running)?;
                    }
                    Action::Restart => {
                        self.set_state(runtime, container_id, State::Restarting)?;
                        self.set_health(runtime, container_id, Health::Starting)?;
                    }
                    Action::Remove => {
                        self.set_state(runtime, container_id, State::Removing)?;
                        self.set_health(runtime, container_id, Health::Empty)?;
                    }
                    Action::Rename => {
                        if let Some(name) = &container_event.name
                            && let Some(container) = self
                                .containers
                                .get_mut(runtime)
                                .and_then(|containers| containers.get_mut(container_id))
                        {
                            container.name = name.clone();
                        }
                    }
                    Action::Oom => {
                        self.set_state(runtime, container_id, State::Exited)?;
                        self.set_health(runtime, container_id, Health::Empty)?;
                    }
                    Action::Health(health) => {
                        self.set_health(runtime, container_id, health)?;
                    }
                    Action::Unknown(_) => {}
                }
            }
            CollectorEvent::RuntimeUp { runtime } => {
                self.runtimes.insert(runtime, RuntimeStatus::Up);
            }
            CollectorEvent::RuntimeDown { runtime, .. } => {
                self.runtimes.insert(runtime, RuntimeStatus::Down);
            }
        }

        Ok(())
    }

    fn set_state(
        &mut self,
        runtime: &Runtime,
        container_id: &str,
        state: State,
    ) -> Result<(), AppError> {
        self.container_mut(runtime, container_id)?.state = state;

        Ok(())
    }

    fn set_health(
        &mut self,
        runtime: &Runtime,
        container_id: &str,
        health: Health,
    ) -> Result<(), AppError> {
        self.container_mut(runtime, container_id)?.health = health;

        Ok(())
    }

    fn container_mut(
        &mut self,
        runtime: &Runtime,
        container_id: &str,
    ) -> Result<&mut Container, AppError> {
        self.containers
            .get_mut(runtime)
            .and_then(|containers| containers.get_mut(container_id))
            .ok_or_else(|| AppError::ContainerNotFound {
                runtime: *runtime,
                container_id: container_id.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContainerEvent;

    fn container(runtime: Runtime, id: &str) -> Container {
        Container {
            id: id.to_owned(),
            name: format!("name-{id}"),
            image: "image:latest".to_owned(),
            runtime,
            state: State::Exited,
            health: Health::Healthy,
        }
    }

    fn app_with(runtime: Runtime, containers: Vec<Container>) -> App {
        let mut app = App::new(vec![runtime]);
        app.apply_event(CollectorEvent::Snapshot {
            runtime,
            containers,
        })
        .unwrap();
        app
    }

    fn event(runtime: Runtime, id: &str, action: Action) -> CollectorEvent {
        CollectorEvent::Container(ContainerEvent {
            runtime,
            id: id.to_owned(),
            name: None,
            action,
        })
    }

    fn assert_container(app: &App, runtime: Runtime, id: &str, state: State, health: Health) {
        let container = &app.containers[&runtime][id];
        assert_eq!(container.state, state);
        assert_eq!(container.health, health);
    }

    #[test]
    fn new_marks_runtimes_down() {
        let app = App::new(vec![Runtime::Docker, Runtime::Podman]);

        assert_eq!(app.runtimes[&Runtime::Docker], RuntimeStatus::Down);
        assert_eq!(app.runtimes[&Runtime::Podman], RuntimeStatus::Down);
    }

    #[test]
    fn snapshot_indexes_containers_by_id() {
        let app = app_with(
            Runtime::Docker,
            vec![
                container(Runtime::Docker, "c1"),
                container(Runtime::Docker, "c2"),
            ],
        );

        let containers = &app.containers[&Runtime::Docker];
        assert_eq!(containers.len(), 2);
        assert_eq!(containers["c1"].name, "name-c1");
        assert_eq!(containers["c2"].name, "name-c2");
    }

    #[test]
    fn snapshot_replaces_existing_containers() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "old")]);

        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "new")],
        })
        .unwrap();

        let containers = &app.containers[&Runtime::Docker];
        assert!(!containers.contains_key("old"));
        assert!(containers.contains_key("new"));
    }

    #[test]
    fn snapshot_only_replaces_target_runtime() {
        let mut app = App::new(vec![Runtime::Docker, Runtime::Podman]);

        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "docker-1")],
        })
        .unwrap();
        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Podman,
            containers: vec![container(Runtime::Podman, "podman-1")],
        })
        .unwrap();

        assert!(app.containers[&Runtime::Docker].contains_key("docker-1"));
        assert!(app.containers[&Runtime::Podman].contains_key("podman-1"));
    }

    #[test]
    fn lifecycle_actions_update_state_and_health() {
        let cases = [
            (Action::Start, State::Running, Health::Starting),
            (Action::Stop, State::Stopping, Health::Healthy),
            (Action::Die, State::Exited, Health::Empty),
            (Action::Pause, State::Paused, Health::Healthy),
            (Action::Unpause, State::Running, Health::Healthy),
            (Action::Restart, State::Restarting, Health::Starting),
            (Action::Remove, State::Removing, Health::Empty),
            (Action::Oom, State::Exited, Health::Empty),
        ];

        for (action, state, health) in cases {
            let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

            app.apply_event(event(Runtime::Docker, "c1", action))
                .unwrap();

            assert_container(&app, Runtime::Docker, "c1", state, health);
        }
    }

    #[test]
    fn health_action_updates_health_only() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(
            Runtime::Docker,
            "c1",
            Action::Health(Health::Unhealthy),
        ))
        .unwrap();

        assert_container(
            &app,
            Runtime::Docker,
            "c1",
            State::Exited,
            Health::Unhealthy,
        );
    }

    #[test]
    fn rename_updates_container_name() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(CollectorEvent::Container(ContainerEvent {
            runtime: Runtime::Docker,
            id: "c1".to_owned(),
            name: Some("renamed".to_owned()),
            action: Action::Rename,
        }))
        .unwrap();

        assert_eq!(app.containers[&Runtime::Docker]["c1"].name, "renamed");
    }

    #[test]
    fn rename_without_name_keeps_existing_name() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(Runtime::Docker, "c1", Action::Rename))
            .unwrap();

        assert_eq!(app.containers[&Runtime::Docker]["c1"].name, "name-c1");
    }

    #[test]
    fn unknown_action_leaves_container_unchanged() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(
            Runtime::Docker,
            "c1",
            Action::Unknown("exec_start".to_owned()),
        ))
        .unwrap();

        assert_container(&app, Runtime::Docker, "c1", State::Exited, Health::Healthy);
    }

    #[test]
    fn event_for_unknown_container_returns_error() {
        let mut app = App::new(vec![Runtime::Docker]);

        let error = app
            .apply_event(event(Runtime::Docker, "missing", Action::Start))
            .unwrap_err();

        match error {
            AppError::ContainerNotFound {
                runtime,
                container_id,
            } => {
                assert_eq!(runtime, Runtime::Docker);
                assert_eq!(container_id, "missing");
            }
        }
    }

    #[test]
    fn runtime_events_update_status() {
        let mut app = App::new(vec![Runtime::Docker]);

        app.apply_event(CollectorEvent::RuntimeUp {
            runtime: Runtime::Docker,
        })
        .unwrap();
        assert_eq!(app.runtimes[&Runtime::Docker], RuntimeStatus::Up);

        app.apply_event(CollectorEvent::RuntimeDown {
            runtime: Runtime::Docker,
            reason: "boom".to_owned(),
        })
        .unwrap();
        assert_eq!(app.runtimes[&Runtime::Docker], RuntimeStatus::Down);
    }
}
