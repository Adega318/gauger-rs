use crate::model::{Container, ContainerEvent, ResourceUsage, Runtime};

#[derive(Debug)]
pub enum CollectorEvent {
    Snapshot {
        runtime: Runtime,
        containers: Vec<Container>,
    },
    Container(ContainerEvent),
    Stats {
        runtime: Runtime,
        samples: Vec<(String, ResourceUsage)>,
    },
    RuntimeUp {
        runtime: Runtime,
    },
    RuntimeDown {
        runtime: Runtime,
        reason: String,
    },
}
