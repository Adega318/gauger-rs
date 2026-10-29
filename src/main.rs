use std::error::Error;

use futures_util::StreamExt;

use self::runtime::docker::DockerRuntime;

mod model;
mod runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let runtime = DockerRuntime::connect()?;

    let containers = runtime.list_containers().await?;

    for container in containers {
        println!("{container:?}");
    }

    let mut events = runtime.get_event_stream();
    //tokio::pin!(events);

    while let Some(result) = events.next().await {
        match result {
            Ok(ev) => {
                let action = ev.action.unwrap_or_default();
                let attrs = ev.actor.and_then(|a| a.attributes).unwrap_or_default();
                let name = attrs.get("name").map(String::as_str).unwrap_or("?");

                if action == "start" {
                    println!("started: {name}");
                } else if action == "die" {
                    let code = attrs.get("exitCode").map(String::as_str).unwrap_or("?");
                    println!("died: {name} (exit {code})");
                } else if let Some(status) = action.strip_prefix("health_status: ") {
                    println!("health: {name} is {status}");
                }
            }
            Err(e) => {
                eprintln!("stream error: {e}");
                break;
            }
        }
    }

    Ok(())
}
