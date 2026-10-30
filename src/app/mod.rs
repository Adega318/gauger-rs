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
