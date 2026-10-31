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

    use std::os::unix::fs::MetadataExt;

    let uid = std::fs::metadata("/proc/self")
        .ok()
        .map(|metadata| metadata.uid());

    let mut rootless = Vec::new();
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        rootless.push(format!("{runtime_dir}/podman/podman.sock"));
    }
    if let Some(uid) = uid {
        rootless.push(format!("/run/user/{uid}/podman/podman.sock"));
    }

    select_socket(
        rootless,
        "/run/podman/podman.sock".to_string(),
        uid.unwrap_or(u32::MAX),
        |path| Path::new(path).exists(),
    )
    .ok_or_else(|| {
        "no Podman socket found (checked CONTAINER_HOST, XDG runtime socket, and /run/user/$UID); \
         start a rootless socket with `systemctl --user enable --now podman.socket` or set CONTAINER_HOST"
            .to_string()
    })
}

#[cfg(unix)]
fn select_socket(
    rootless: impl IntoIterator<Item = String>,
    system: String,
    uid: u32,
    exists: impl Fn(&str) -> bool,
) -> Option<String> {
    if let Some(path) = rootless.into_iter().find(|path| exists(path)) {
        return Some(path);
    }

    (uid == 0 && exists(&system)).then_some(system)
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn prefers_first_existing_rootless_socket() {
        let rootless = vec![
            "/run/user/1000/podman/podman.sock".to_string(),
            "/tmp/podman.sock".to_string(),
        ];

        let socket = select_socket(
            rootless,
            "/run/podman/podman.sock".to_string(),
            1000,
            |path| path == "/tmp/podman.sock",
        );

        assert_eq!(socket.as_deref(), Some("/tmp/podman.sock"));
    }

    #[test]
    fn non_root_does_not_fall_back_to_system_socket() {
        let socket = select_socket(
            Vec::new(),
            "/run/podman/podman.sock".to_string(),
            1000,
            |_| true,
        );

        assert_eq!(socket, None);
    }

    #[test]
    fn root_falls_back_to_system_socket() {
        let socket = select_socket(Vec::new(), "/run/podman/podman.sock".to_string(), 0, |_| {
            true
        });

        assert_eq!(socket.as_deref(), Some("/run/podman/podman.sock"));
    }
}
