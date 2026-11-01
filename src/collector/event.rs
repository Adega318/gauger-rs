use crate::model::{Container, ContainerEvent, Runtime};

#[derive(Debug)]
pub enum CollectorEvent {
    Snapshot {
        runtime: Runtime,
        containers: Vec<Container>,
    },
    Container(ContainerEvent),
    RuntimeUp {
        runtime: Runtime,
    },
    RuntimeDown {
        runtime: Runtime,
        reason: String,
    },
}
