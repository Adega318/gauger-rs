pub mod display;
pub mod from;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Health {
    Starting,
    Healthy,
    Unhealthy,
    #[default]
    Empty,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerEvent {
    pub runtime: Runtime,
    pub id: String,
    pub name: Option<String>,
    pub action: Action,
}
