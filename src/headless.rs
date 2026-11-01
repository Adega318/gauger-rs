use crate::collector::{Collector, CollectorEvent};
use crate::connect_runtimes;
use crate::error::GaugerError;

pub async fn run_headless() -> Result<(), GaugerError> {
    let clients = connect_runtimes()?;
    let mut collector = Collector::new(clients).spawn();

    loop {
        let terminated = tokio::signal::ctrl_c();
        tokio::select! {
            Some(event) = collector.recv() => {
                match &event {
                    CollectorEvent::Snapshot {
                        runtime,
                        containers,
                    } => {
                        println!(
                            "Snapshot of {runtime}:{}",
                            if containers.is_empty() {
                                " (empty)"
                            } else {
                                ""
                            }
                        );

                        for container in containers {
                            println!("  {container}");
                        }
                    }
                    CollectorEvent::Container(event) => {
                        println!("Container: {}", event);
                    }
                    CollectorEvent::RuntimeUp { runtime } => {
                        println!("up: {}", runtime)
                    }
                    CollectorEvent::RuntimeDown { runtime, reason } => {
                        println!("down: {}, {}", runtime, reason)
                    }
                }
            }
            _ = terminated => break Ok(()),
        }
    }
}
