use std::error::Error;

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

    Ok(())
}
