use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::{AppError, ContainerKey, RuntimeState, RuntimeStatus, short_id};
use crate::collector::{CollectorEvent, CollectorHandle};
use crate::model::{Action, ContainerEvent, Runtime};

type Names = BTreeMap<ContainerKey, String>;

/// Stream container listings and lifecycle events without taking over the terminal.
pub async fn run(
    mut collector: CollectorHandle,
    runtimes: &BTreeMap<Runtime, RuntimeState>,
) -> Result<(), AppError> {
    for (runtime, state) in runtimes {
        if let RuntimeStatus::Unavailable { reason } = &state.status {
            eprintln!("{runtime} unavailable: {reason}");
        }
    }

    let mut out = io::stdout();
    let mut names = Names::new();

    loop {
        tokio::select! {
            event = collector.recv() => {
                let Some(event) = event else {
                    eprintln!("event collector stopped");
                    return Ok(());
                };

                match &event {
                    CollectorEvent::RuntimeUp { runtime } => eprintln!("{runtime} up"),
                    CollectorEvent::RuntimeDown { runtime, reason } => {
                        eprintln!("{runtime} down: {reason}");
                    }
                    CollectorEvent::Snapshot { .. } | CollectorEvent::Container(_) => {}
                }

                match emit(&mut out, &timestamp(), event, &mut names) {
                    Ok(()) => {}
                    // Downstream commands such as `head` close the pipe early.
                    Err(err) if err.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
                    Err(err) => return Err(err.into()),
                }
            }
            result = shutdown() => {
                result?;
                return Ok(());
            }
        }
    }
}

fn emit(
    out: &mut impl Write,
    time: &str,
    event: CollectorEvent,
    names: &mut Names,
) -> io::Result<()> {
    match event {
        CollectorEvent::Snapshot {
            runtime,
            containers,
        } => {
            names.retain(|(known_runtime, _), _| *known_runtime != runtime);

            for container in containers {
                writeln!(
                    out,
                    "{time}  {runtime:<7}  {:<8} {:<12}  {:<24}  {:<10}  {:<9}  {}",
                    "list",
                    short_id(&container.id),
                    container.name,
                    container.state,
                    container.health,
                    container.image,
                )?;
                names.insert((runtime, container.id), container.name);
            }
        }
        CollectorEvent::Container(event) => emit_container(out, time, event, names)?,
        CollectorEvent::RuntimeUp { .. } | CollectorEvent::RuntimeDown { .. } => {}
    }

    out.flush()
}

fn emit_container(
    out: &mut impl Write,
    time: &str,
    event: ContainerEvent,
    names: &mut Names,
) -> io::Result<()> {
    let key = (event.runtime, event.id.clone());

    if let Some(name) = &event.name {
        names.insert(key.clone(), name.clone());
    }

    let name = event
        .name
        .as_ref()
        .or_else(|| names.get(&key))
        .cloned()
        .unwrap_or_else(|| short_id(&event.id));
    let id = short_id(&event.id);
    let runtime = event.runtime;

    match &event.action {
        Action::Health(health) => writeln!(
            out,
            "{time}  {runtime:<7}  {:<8} {id:<12}  {name}  {}",
            event.action, *health
        )?,
        Action::Unknown(raw) => writeln!(
            out,
            "{time}  {runtime:<7}  {:<8} {id:<12}  {name}  {raw}",
            "unknown"
        )?,
        _ => writeln!(
            out,
            "{time}  {runtime:<7}  {:<8} {id:<12}  {name}",
            event.action
        )?,
    }

    if matches!(&event.action, Action::Remove) {
        names.remove(&key);
    }

    Ok(())
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();

    timestamp_at(seconds)
}

fn timestamp_at(epoch_seconds: u64) -> String {
    let seconds = epoch_seconds % 86_400;

    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        (seconds % 3_600) / 60,
        seconds % 60
    )
}

async fn shutdown() -> io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;

        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Container, Health, State};

    const TIME: &str = "12:00:00";

    fn container(runtime: Runtime, id: &str, name: &str, state: State) -> Container {
        Container {
            id: id.to_owned(),
            name: name.to_owned(),
            image: format!("{name}:latest"),
            runtime,
            state,
            health: Health::Unknown,
        }
    }

    fn snapshot(runtime: Runtime, containers: Vec<Container>) -> CollectorEvent {
        CollectorEvent::Snapshot {
            runtime,
            containers,
        }
    }

    fn event(runtime: Runtime, id: &str, name: Option<&str>, action: Action) -> CollectorEvent {
        CollectorEvent::Container(ContainerEvent {
            runtime,
            id: id.to_owned(),
            name: name.map(str::to_owned),
            action,
        })
    }

    fn emit_to_string(event: CollectorEvent, names: &mut Names) -> String {
        let mut out = Vec::new();
        emit(&mut out, TIME, event, names).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn prints_snapshot_rows_and_remembers_names() {
        let mut names = Names::new();
        let output = emit_to_string(
            snapshot(
                Runtime::Docker,
                vec![container(
                    Runtime::Docker,
                    "05ae208348f4f09389f027adb597b16a1",
                    "postgres",
                    State::Running,
                )],
            ),
            &mut names,
        );

        assert!(output.contains("12:00:00  docker   list"), "{output:?}");
        assert!(output.contains("05ae208348f4"));
        assert!(output.contains("postgres"));
        assert!(output.contains("running"));
        assert!(output.contains("postgres:latest"));
        assert_eq!(
            names.get(&(
                Runtime::Docker,
                "05ae208348f4f09389f027adb597b16a1".to_owned()
            )),
            Some(&"postgres".to_owned())
        );
    }

    #[test]
    fn resolves_names_for_events_without_one() {
        let mut names = Names::new();
        emit_to_string(
            snapshot(
                Runtime::Docker,
                vec![container(
                    Runtime::Docker,
                    "abc123def456789",
                    "web",
                    State::Running,
                )],
            ),
            &mut names,
        );

        let output = emit_to_string(
            event(Runtime::Docker, "abc123def456789", None, Action::Start),
            &mut names,
        );

        assert!(output.contains("start"));
        assert!(output.contains("web"));
    }

    #[test]
    fn create_records_the_name_from_the_event() {
        let mut names = Names::new();

        let output = emit_to_string(
            event(
                Runtime::Podman,
                "abc123def456789",
                Some("api"),
                Action::Create,
            ),
            &mut names,
        );

        assert!(output.contains("podman"));
        assert!(output.contains("create"));
        assert!(output.contains("api"));
        assert_eq!(
            names.get(&(Runtime::Podman, "abc123def456789".to_owned())),
            Some(&"api".to_owned())
        );
    }

    #[test]
    fn remove_forgets_the_name() {
        let mut names = Names::new();
        emit_to_string(
            snapshot(
                Runtime::Docker,
                vec![container(
                    Runtime::Docker,
                    "abc123def456789",
                    "web",
                    State::Running,
                )],
            ),
            &mut names,
        );

        let output = emit_to_string(
            event(Runtime::Docker, "abc123def456789", None, Action::Remove),
            &mut names,
        );

        assert!(output.contains("remove"));
        assert!(output.contains("web"));
        assert!(!names.contains_key(&(Runtime::Docker, "abc123def456789".to_owned())));

        let output = emit_to_string(
            event(Runtime::Docker, "abc123def456789", None, Action::Start),
            &mut names,
        );
        assert!(output.contains("abc123def456"));
        assert!(!output.contains("web"));
    }

    #[test]
    fn snapshot_only_replaces_its_own_runtime() {
        let mut names = Names::new();
        emit_to_string(
            snapshot(
                Runtime::Docker,
                vec![container(
                    Runtime::Docker,
                    "docker000000",
                    "docker-app",
                    State::Running,
                )],
            ),
            &mut names,
        );
        emit_to_string(
            snapshot(
                Runtime::Podman,
                vec![container(
                    Runtime::Podman,
                    "podman000000",
                    "podman-app",
                    State::Running,
                )],
            ),
            &mut names,
        );

        emit_to_string(
            snapshot(
                Runtime::Docker,
                vec![container(
                    Runtime::Docker,
                    "docker111111",
                    "docker-new",
                    State::Running,
                )],
            ),
            &mut names,
        );

        assert!(!names.contains_key(&(Runtime::Docker, "docker000000".to_owned())));
        assert!(names.contains_key(&(Runtime::Podman, "podman000000".to_owned())));
    }

    #[test]
    fn health_and_unknown_events_include_their_detail() {
        let mut names = Names::new();

        let health = emit_to_string(
            event(
                Runtime::Docker,
                "abc123def456789",
                None,
                Action::Health(Health::Unhealthy),
            ),
            &mut names,
        );
        assert!(health.contains("health"));
        assert!(health.contains("unhealthy"));

        let unknown = emit_to_string(
            event(
                Runtime::Docker,
                "abc123def456789",
                None,
                Action::Unknown("some-action".to_owned()),
            ),
            &mut names,
        );
        println!("unknown: {}", unknown);
        assert!(unknown.contains("unknown"));
        assert_eq!(unknown.matches("some-action").count(), 1);
    }

    #[test]
    fn formats_timestamps_as_utc() {
        assert_eq!(timestamp_at(0), "00:00:00");
        assert_eq!(timestamp_at(3_661), "01:01:01");
        assert_eq!(timestamp_at(86_399), "23:59:59");
        assert_eq!(timestamp_at(90_061), "01:01:01");
    }
}
