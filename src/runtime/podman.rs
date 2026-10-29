use std::env;
use std::path::Path;

use bollard::Docker;

use crate::model::Runtime;

use super::RuntimeClient;
use super::error::RuntimeError;

pub struct PodmanRuntime;

impl PodmanRuntime {
    pub fn connect() -> Result<RuntimeClient, RuntimeError> {
        let path = socket_path().map_err(|reason| RuntimeError::ConnectionError {
            runtime: Runtime::Podman.to_string(),
            reason,
        })?;

        let client =
            Docker::connect_with_unix(&path, 120, bollard::API_DEFAULT_VERSION).map_err(|err| {
                RuntimeError::ConnectionError {
                    runtime: Runtime::Podman.to_string(),
                    reason: err.to_string(),
                }
            })?;

        Ok(RuntimeClient {
            runtime: Runtime::Podman,
            client,
        })
    }
}

fn socket_path() -> Result<String, String> {
    if let Ok(host) = env::var("CONTAINER_HOST") {
        let path = host
            .strip_prefix("unix://")
            .ok_or_else(|| "CONTAINER_HOST must use a unix:// socket".to_string())?;
        return Ok(path.to_string());
    }

    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        let path = format!("{runtime_dir}/podman/podman.sock");
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    if let Ok(metadata) = std::fs::metadata("/proc/self") {
        use std::os::unix::fs::MetadataExt;

        let path = format!("/run/user/{}/podman/podman.sock", metadata.uid());
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    let system_socket = "/run/podman/podman.sock";
    if Path::new(system_socket).exists() {
        return Ok(system_socket.to_string());
    }

    Err(
        "no Podman socket found (checked CONTAINER_HOST, XDG runtime socket, /run/user/$UID, and /run/podman)"
            .to_string(),
    )
}
