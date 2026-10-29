use std::error::Error;

use futures_util::StreamExt;

use self::runtime::RuntimeClient;
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

    while let Some(result) = events.next().await {
        match result {
            Ok(ev) => eprintln!("{:?}", RuntimeClient::handle_container_event(ev)),
            Err(e) => {
                eprintln!("stream error: {e}");
                break;
            }
        }
    }

    Ok(())
}
