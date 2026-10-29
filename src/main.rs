use std::error::Error;
use std::io;

use self::collector::{Collector, CollectorEvent};
use self::runtime::RuntimeClient;
use self::runtime::docker::DockerRuntime;
use self::runtime::podman::PodmanRuntime;

mod collector;
mod model;
mod runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut clients: Vec<RuntimeClient> = Vec::new();

    match DockerRuntime::connect() {
        Ok(client) => clients.push(client),
        Err(err) => eprintln!("docker unavailable: {err}"),
    }

    match PodmanRuntime::connect() {
        Ok(client) => clients.push(client),
        Err(err) => eprintln!("podman unavailable: {err}"),
    }

    if clients.is_empty() {
        return Err(
            io::Error::new(io::ErrorKind::NotFound, "no container runtime available").into(),
        );
    }

    let mut collector = Collector::new(clients).spawn();

    while let Some(event) = collector.recv().await {
        match event {
            CollectorEvent::Snapshot {
                runtime,
                containers,
            } => {
                println!("{runtime}: {} containers", containers.len());
                for container in containers {
                    println!(
                        "  {} [{}] {} {} {:?} {:?}",
                        container.name,
                        container.id,
                        container.image,
                        container.runtime,
                        container.state,
                        container.health
                    );
                }
            }
            CollectorEvent::Container(event) => println!("{event:?}"),
            CollectorEvent::RuntimeUp { runtime } => println!("{runtime}: connected"),
            CollectorEvent::RuntimeDown { runtime, reason } => {
                eprintln!("{runtime}: disconnected: {reason}");
            }
        }
    }

    Ok(())
}
