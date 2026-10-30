use std::env;

#[cfg(unix)]
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

        let client = connect_podman(&path).map_err(|err| RuntimeError::ConnectionError {
            runtime: Runtime::Podman.to_string(),
            reason: err.to_string(),
        })?;

        Ok(RuntimeClient {
            runtime: Runtime::Podman,
            client,
        })
    }
}

/// Discover the Podman control socket on Unix.
#[cfg(unix)]
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

/// Discover the Podman named pipe on Windows.
#[cfg(windows)]
fn socket_path() -> Result<String, String> {
    if let Ok(host) = env::var("CONTAINER_HOST") {
        let path = host
            .strip_prefix("npipe://")
            .ok_or_else(|| "CONTAINER_HOST must use an npipe:// socket on Windows".to_string())?;
        return Ok(path.to_string());
    }

    // Podman for Windows exposes its API over a named pipe at this well-known location.
    let default_pipe = r"\\.\pipe\podman";
    Ok(default_pipe.to_string())
}

/// Connect bollard to the discovered Unix socket.
#[cfg(unix)]
fn connect_podman(path: &str) -> Result<Docker, bollard::errors::Error> {
    Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
}

/// Connect bollard to the discovered Windows named pipe.
#[cfg(windows)]
fn connect_podman(path: &str) -> Result<Docker, bollard::errors::Error> {
    Docker::connect_with_named_pipe(path, 120, bollard::API_DEFAULT_VERSION)
}
