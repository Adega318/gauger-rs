use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::TableState;
use thiserror::Error;

use crate::collector::CollectorEvent;
use crate::model::{Action, Container, ContainerEvent, Health, Runtime, State};

pub use self::filter::Filter;
pub use self::input::Input;
pub use self::sort::Sort;

pub mod filter;
pub mod input;
pub mod sort;

/// Containers are only unique within one engine, so the runtime is part of the key.
pub type ContainerKey = (Runtime, String);

pub const REFRESH_DEBOUNCE: Duration = Duration::from_millis(500);
pub const RESYNC_INTERVAL: Duration = Duration::from_secs(60);
pub const NOTICE_TTL: Duration = Duration::from_secs(4);

#[derive(Debug, Clone)]
pub enum RuntimeStatus {
    Connecting,
    Up,
    Down,
    Unavailable { reason: String },
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub status: RuntimeStatus,
    pub last_snapshot: Option<Instant>,
}

impl RuntimeState {
    pub fn connecting() -> Self {
        Self {
            status: RuntimeStatus::Connecting,
            last_snapshot: None,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: RuntimeStatus::Unavailable {
                reason: reason.into(),
            },
            last_snapshot: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub message: String,
    pub error: bool,
    pub expires: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Details(ContainerKey),
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Refresh(Runtime),
    RefreshAll,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("no container runtime available")]
    NoRuntime,
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct App {
    pub containers: BTreeMap<ContainerKey, Container>,
    pub runtimes: BTreeMap<Runtime, RuntimeState>,
    pub visible: Vec<ContainerKey>,
    pub selected: usize,
    pub table_state: TableState,
    pub filter: Filter,
    pub sort: Sort,
    pub mode: Mode,
    pub should_quit: bool,
    pub notice: Option<Notice>,
    pub last_event: Option<(ContainerKey, Action, Instant)>,
    pending_refresh: BTreeMap<Runtime, Instant>,
    last_resync: Instant,
    effects: Vec<Effect>,
}

impl App {
    pub fn new(runtimes: BTreeMap<Runtime, RuntimeState>) -> Self {
        let mut app = Self {
            containers: BTreeMap::new(),
            runtimes,
            visible: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            filter: Filter::default(),
            sort: Sort::default(),
            mode: Mode::Normal,
            should_quit: false,
            notice: None,
            last_event: None,
            pending_refresh: BTreeMap::new(),
            last_resync: Instant::now(),
            effects: Vec::new(),
        };
        app.rebuild_view();

        app
    }

    pub fn apply(&mut self, event: CollectorEvent) {
        match event {
            CollectorEvent::Snapshot {
                runtime,
                containers,
            } => {
                self.containers
                    .retain(|(container_runtime, _), _| *container_runtime != runtime);

                for container in containers {
                    self.containers
                        .insert((runtime, container.id.clone()), container);
                }

                self.set_status(runtime, RuntimeStatus::Up);
                if let Some(state) = self.runtimes.get_mut(&runtime) {
                    state.last_snapshot = Some(Instant::now());
                }

                if matches!(&self.mode, Mode::Details(key) if !self.containers.contains_key(key)) {
                    self.mode = Mode::Normal;
                }

                self.rebuild_view();
            }
            CollectorEvent::Container(event) => self.apply_container(event),
            CollectorEvent::RuntimeUp { runtime } => {
                self.set_status(runtime, RuntimeStatus::Up);
                if let Some(state) = self.runtimes.get_mut(&runtime)
                    && state.last_snapshot.is_none()
                {
                    state.last_snapshot = Some(Instant::now());
                }
            }
            CollectorEvent::RuntimeDown { runtime, reason: _ } => {
                self.set_status(runtime, RuntimeStatus::Down);
            }
        }
    }

    fn apply_container(&mut self, event: ContainerEvent) {
        let key = (event.runtime, event.id.clone());
        let now = Instant::now();

        match &event.action {
            Action::Create => {
                self.containers
                    .entry(key.clone())
                    .or_insert_with(|| placeholder(&event, State::Created));
                self.schedule_refresh(event.runtime, now);
            }
            Action::Start => {
                self.set_state(&event, State::Running);
                self.mark_health_starting(&key);
            }
            Action::Stop => self.set_state(&event, State::Stopping),
            Action::Die => {
                self.set_state(&event, State::Exited);
                self.set_health(&key, &Health::Empty);
            }
            Action::Pause => {
                self.set_state(&event, State::Paused);
            }
            Action::Unpause => {
                self.set_state(&event, State::Running);
                self.mark_health_starting(&key);
            }
            Action::Restart => {
                self.set_state(&event, State::Restarting);
                self.mark_health_starting(&key);
            }
            Action::Remove => {
                self.set_state(&event, State::Removing);
                self.set_health(&key, &Health::Empty);
                self.schedule_refresh(event.runtime, now);
            }
            Action::Rename => {
                if let Some(name) = &event.name
                    && let Some(container) = self.containers.get_mut(&key)
                {
                    container.name = name.clone();
                }
            }
            Action::Oom => {
                self.set_state(&event, State::Exited);
                self.set_health(&key, &Health::Empty);
                let name = self.display_name(&key);
                self.set_notice(format!("{name} was OOM-killed"), true);
            }
            Action::Health(health) => {
                if self.get_state(&key) == State::Restarting {
                    self.set_state(&event, State::Running)
                }
                // Podman reports bare `health_status` actions and a failed
                // inspect leaves `Unknown`; neither should erase a value we
                // already know.
                if *health != Health::Unknown {
                    self.set_health(&key, health);
                }
            }
            Action::Unknown(_) => return,
        }

        self.last_event = Some((key, event.action, now));
        self.rebuild_view();
    }

    fn get_state(&self, key: &(Runtime, String)) -> State {
        self.containers
            .get(key)
            .map(|container| container.state)
            .unwrap_or(State::Unknown)
    }

    fn set_state(&mut self, event: &ContainerEvent, state: State) {
        let key = (event.runtime, event.id.clone());
        let container = self
            .containers
            .entry(key)
            .or_insert_with(|| placeholder(event, state));
        container.state = state;
    }

    fn set_status(&mut self, runtime: Runtime, status: RuntimeStatus) {
        self.runtimes
            .entry(runtime)
            .or_insert_with(RuntimeState::connecting)
            .status = status;
    }

    fn set_health(&mut self, key: &(Runtime, String), health: &Health) {
        if let Some(container) = self.containers.get_mut(key) {
            container.health = *health;
        }
    }

    fn mark_health_starting(&mut self, key: &(Runtime, String)) {
        // Containers without a healthcheck also report `Unknown`, so only
        // transition health values that were actually known.
        if let Some(container) = self.containers.get_mut(key)
            && !matches!(container.health, Health::Unknown | Health::Empty)
        {
            container.health = Health::Starting;
        }
    }

    fn schedule_refresh(&mut self, runtime: Runtime, now: Instant) {
        self.pending_refresh.insert(runtime, now + REFRESH_DEBOUNCE);
    }

    fn rebuild_view(&mut self) {
        let previous = self.visible.get(self.selected).cloned();

        let filter = &self.filter;
        let sort = &self.sort;
        let mut containers: Vec<&Container> = self
            .containers
            .values()
            .filter(|container| filter.matches(container))
            .collect();
        containers.sort_by(|a, b| sort.compare(a, b));

        self.visible = containers
            .iter()
            .map(|container| (container.runtime, container.id.clone()))
            .collect();

        self.selected = previous
            .and_then(|key| self.visible.iter().position(|visible| *visible == key))
            .unwrap_or_else(|| self.selected.min(self.visible.len().saturating_sub(1)));

        if self.visible.is_empty() {
            self.selected = 0;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return;
        }

        if let Some(input) = input::translate(key, &self.mode) {
            self.handle_input(input);
        }
    }

    fn handle_input(&mut self, input: Input) {
        match input {
            Input::Quit => self.should_quit = true,
            Input::Down => self.move_selection(1),
            Input::Up => self.move_selection(-1),
            Input::Top => self.selected = 0,
            Input::Bottom => self.selected = self.visible.len().saturating_sub(1),
            Input::PageDown => self.move_selection(10),
            Input::PageUp => self.move_selection(-10),
            Input::StartFilter => {
                self.filter.begin();
                self.mode = Mode::Filter;
            }
            Input::ApplyFilter => {
                self.filter.commit();
                self.mode = Mode::Normal;
            }
            Input::CancelFilter => {
                self.filter.cancel();
                self.mode = Mode::Normal;
                self.rebuild_view();
            }
            Input::ClearFilter => {
                self.filter.clear();
                self.rebuild_view();
            }
            Input::Char(character) => {
                self.filter.input.push(character);
                self.filter.edit();
                self.rebuild_view();
            }
            Input::Backspace => {
                self.filter.input.pop();
                self.filter.edit();
                self.rebuild_view();
            }
            Input::CycleSort => {
                self.sort.key = self.sort.key.next();
                self.rebuild_view();
            }
            Input::ReverseSort => {
                self.sort.descending = !self.sort.descending;
                self.rebuild_view();
            }
            Input::Refresh => self.effects.push(Effect::RefreshAll),
            Input::OpenDetails => {
                if let Some(key) = self.visible.get(self.selected) {
                    self.mode = Mode::Details(key.clone());
                }
            }
            Input::Help => self.mode = Mode::Help,
            Input::CloseModal => self.mode = Mode::Normal,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }

        let last = self.visible.len() - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last as isize) as usize;
    }

    pub fn tick(&mut self) {
        let now = Instant::now();

        if self
            .notice
            .as_ref()
            .is_some_and(|notice| notice.expires <= now)
        {
            self.notice = None;
        }

        let due: Vec<Runtime> = self
            .pending_refresh
            .iter()
            .filter(|(_, due)| **due <= now)
            .map(|(runtime, _)| *runtime)
            .collect();

        for runtime in due {
            self.pending_refresh.remove(&runtime);
            self.effects.push(Effect::Refresh(runtime));
        }

        if now.duration_since(self.last_resync) >= RESYNC_INTERVAL {
            self.last_resync = now;
            self.effects.push(Effect::RefreshAll);
        }
    }

    pub fn drain_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn set_notice(&mut self, message: impl Into<String>, error: bool) {
        self.notice = Some(Notice {
            message: message.into(),
            error,
            expires: Instant::now() + NOTICE_TTL,
        });
    }

    pub fn collector_stopped(&mut self) {
        self.set_notice("event collector stopped; data is stale", true);
    }

    pub fn input_stopped(&mut self) {
        self.set_notice("terminal input stream ended", true);
    }

    fn display_name(&self, key: &ContainerKey) -> String {
        self.containers
            .get(key)
            .map(|container| container.name.clone())
            .unwrap_or_else(|| short_id(&key.1))
    }
}

fn placeholder(event: &ContainerEvent, state: State) -> Container {
    Container {
        id: event.id.clone(),
        name: event.name.clone().unwrap_or_else(|| short_id(&event.id)),
        image: "unknown".to_owned(),
        runtime: event.runtime,
        state,
        health: Health::Unknown,
    }
}

pub fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContainerEvent;

    fn app() -> App {
        let runtimes = BTreeMap::from([
            (Runtime::Docker, RuntimeState::connecting()),
            (Runtime::Podman, RuntimeState::connecting()),
        ]);
        App::new(runtimes)
    }

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

    fn event(runtime: Runtime, id: &str, action: Action) -> CollectorEvent {
        CollectorEvent::Container(ContainerEvent {
            runtime,
            id: id.to_owned(),
            name: None,
            action,
        })
    }

    #[test]
    fn snapshot_replaces_only_its_runtime() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "docker-a", State::Running)],
        });
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Podman,
            containers: vec![container(Runtime::Podman, "b", "podman-b", State::Running)],
        });

        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "c", "docker-c", State::Exited)],
        });

        assert_eq!(app.containers.len(), 2);
        assert!(
            app.containers
                .contains_key(&(Runtime::Docker, "c".to_owned()))
        );
        assert!(
            app.containers
                .contains_key(&(Runtime::Podman, "b".to_owned()))
        );
        assert!(
            !app.containers
                .contains_key(&(Runtime::Docker, "a".to_owned()))
        );
    }

    #[test]
    fn replayed_create_does_not_downgrade_running() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });

        app.apply(event(Runtime::Docker, "a", Action::Create));

        assert_eq!(
            app.containers
                .get(&(Runtime::Docker, "a".to_owned()))
                .unwrap()
                .state,
            State::Running
        );
    }

    #[test]
    fn lifecycle_events_update_state() {
        let mut app = app();
        app.apply(event(Runtime::Docker, "a", Action::Create));
        assert_eq!(
            app.containers
                .get(&(Runtime::Docker, "a".to_owned()))
                .unwrap()
                .state,
            State::Created
        );

        app.apply(event(Runtime::Docker, "a", Action::Start));
        assert_eq!(
            app.containers
                .get(&(Runtime::Docker, "a".to_owned()))
                .unwrap()
                .state,
            State::Running
        );

        app.apply(event(Runtime::Docker, "a", Action::Pause));
        assert_eq!(
            app.containers
                .get(&(Runtime::Docker, "a".to_owned()))
                .unwrap()
                .state,
            State::Paused
        );

        app.apply(event(Runtime::Docker, "a", Action::Stop));
        assert_eq!(
            app.containers
                .get(&(Runtime::Docker, "a".to_owned()))
                .unwrap()
                .state,
            State::Stopping
        );
    }

    #[test]
    fn unknown_health_event_keeps_last_known_health() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });
        let key = (Runtime::Docker, "a".to_owned());
        app.containers.get_mut(&key).unwrap().health = Health::Healthy;

        app.apply(event(Runtime::Docker, "a", Action::Health(Health::Unknown)));

        assert_eq!(app.containers.get(&key).unwrap().health, Health::Healthy);
    }

    #[test]
    fn known_health_event_updates_health() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });
        let key = (Runtime::Docker, "a".to_owned());
        app.containers.get_mut(&key).unwrap().health = Health::Healthy;

        app.apply(event(
            Runtime::Docker,
            "a",
            Action::Health(Health::Unhealthy),
        ));

        assert_eq!(app.containers.get(&key).unwrap().health, Health::Unhealthy);
    }

    #[test]
    fn start_leaves_unknown_health_alone() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });

        // Containers without a healthcheck also report Unknown.
        app.apply(event(Runtime::Docker, "a", Action::Start));

        assert_eq!(
            app.containers
                .get(&(Runtime::Docker, "a".to_owned()))
                .unwrap()
                .health,
            Health::Unknown
        );
    }

    #[test]
    fn start_marks_known_health_as_starting() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });
        let key = (Runtime::Docker, "a".to_owned());
        app.containers.get_mut(&key).unwrap().health = Health::Healthy;

        app.apply(event(Runtime::Docker, "a", Action::Start));

        assert_eq!(app.containers.get(&key).unwrap().health, Health::Starting);
    }

    #[test]
    fn unknown_health_event_still_finishes_restarting_state() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });
        let key = (Runtime::Docker, "a".to_owned());
        {
            let container = app.containers.get_mut(&key).unwrap();
            container.state = State::Restarting;
            container.health = Health::Healthy;
        }

        app.apply(event(Runtime::Docker, "a", Action::Health(Health::Unknown)));

        let container = app.containers.get(&key).unwrap();
        assert_eq!(container.state, State::Running);
        assert_eq!(container.health, Health::Healthy);
    }

    #[test]
    fn runtime_down_keeps_last_known_containers() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });

        app.apply(CollectorEvent::RuntimeDown {
            runtime: Runtime::Docker,
            reason: "socket closed".to_owned(),
        });

        assert_eq!(app.containers.len(), 1);
        assert!(matches!(
            app.runtimes.get(&Runtime::Docker).unwrap().status,
            RuntimeStatus::Down
        ));
    }

    #[test]
    fn create_schedules_a_debounced_refresh() {
        let mut app = app();
        app.apply(event(Runtime::Docker, "a", Action::Create));

        assert!(app.pending_refresh.contains_key(&Runtime::Docker));

        for due in app.pending_refresh.values_mut() {
            *due = Instant::now();
        }
        app.tick();

        assert_eq!(app.drain_effects(), vec![Effect::Refresh(Runtime::Docker)]);
    }

    #[test]
    fn periodic_resync_emits_refresh_all() {
        let mut app = app();
        app.last_resync = Instant::now() - RESYNC_INTERVAL;

        app.tick();

        assert_eq!(app.drain_effects(), vec![Effect::RefreshAll]);
    }

    #[test]
    fn selection_follows_its_container_when_order_changes() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![
                container(Runtime::Docker, "a", "alpha", State::Running),
                container(Runtime::Docker, "b", "beta", State::Running),
            ],
        });
        let selected_key = app.visible[0].clone();

        app.sort.descending = true;
        app.handle_input(Input::ReverseSort);

        assert_eq!(app.visible[app.selected], selected_key);
    }

    #[test]
    fn filter_restores_previous_query_on_cancel() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![
                container(Runtime::Docker, "a", "alpha", State::Running),
                container(Runtime::Docker, "b", "beta", State::Running),
            ],
        });

        app.handle_input(Input::StartFilter);
        app.handle_input(Input::Char('a'));
        app.handle_input(Input::Char('l'));
        assert_eq!(app.visible.len(), 1);

        app.handle_input(Input::CancelFilter);
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn snapshot_closes_details_for_a_removed_container() {
        let mut app = app();
        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "a", "a", State::Running)],
        });
        app.mode = Mode::Details((Runtime::Docker, "a".to_owned()));

        app.apply(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: Vec::new(),
        });

        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn oom_sets_exited_and_notifies() {
        let mut app = app();
        app.apply(event(Runtime::Docker, "a", Action::Start));
        app.apply(event(Runtime::Docker, "a", Action::Oom));

        let container = app
            .containers
            .get(&(Runtime::Docker, "a".to_owned()))
            .unwrap();
        assert_eq!(container.state, State::Exited);
        assert_eq!(container.health, Health::Empty);
        assert!(app.notice.as_ref().is_some_and(|notice| notice.error));
    }
}
