use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::collector::CollectorEvent;
use crate::model::{Action, Container, Health, ResourceUsage, Runtime, State};

use self::error::AppError;

pub mod error;
pub mod input;
mod search;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Up,
    Down,
}

#[derive(Debug)]
pub enum Effect {
    Refresh,
}

const STATS_STALE_AFTER: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct App {
    runtimes: HashMap<Runtime, RuntimeStatus>,
    containers: HashMap<Runtime, HashMap<String, Container>>,
    usage: HashMap<Runtime, HashMap<String, ResourceUsage>>,
    visible: Vec<(Runtime, String)>,
    search: String,
    searching: bool,
    pub selected: usize,
    pub show_detail: bool,
    effects: Vec<Effect>,
    scheduled_sync: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(runtimes: Vec<Runtime>) -> Self {
        Self {
            runtimes: runtimes
                .into_iter()
                .map(|runtime| (runtime, RuntimeStatus::Down))
                .collect(),
            containers: HashMap::new(),
            usage: HashMap::new(),
            visible: Vec::new(),
            search: String::new(),
            searching: false,
            selected: 0,
            show_detail: false,
            effects: Vec::new(),
            scheduled_sync: false,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {
        if self.scheduled_sync {
            self.effects.push(Effect::Refresh);
            self.scheduled_sync = false;
        }

        self.expire_usage();
    }

    pub fn apply_event(&mut self, collector_event: CollectorEvent) -> Result<(), AppError> {
        let mut rebuild = true;

        match collector_event {
            CollectorEvent::Snapshot {
                runtime,
                containers,
            } => {
                let running: HashSet<String> = containers
                    .iter()
                    .filter(|container| container.state == State::Running)
                    .map(|container| container.id.clone())
                    .collect();

                if let Some(usage) = self.usage.get_mut(&runtime) {
                    usage.retain(|id, _| running.contains(id));
                }

                let containers = containers
                    .into_iter()
                    .map(|container| (container.id.clone(), container))
                    .collect();

                self.containers.insert(runtime, containers);
            }
            CollectorEvent::Container(container_event) => {
                let runtime = &container_event.runtime;
                let container_id = container_event.id.as_str();

                match container_event.action {
                    Action::Create => {
                        let container = Container {
                            id: container_event.id.clone(),
                            name: container_event.name.clone().unwrap_or_default(),
                            image: String::new(),
                            runtime: *runtime,
                            state: State::Created,
                            health: Health::Empty,
                            compose_project: None,
                        };

                        self.containers
                            .entry(*runtime)
                            .or_default()
                            .entry(container.id.clone())
                            .or_insert(container);
                    }
                    Action::Start => {
                        self.set_state(runtime, container_id, State::Running);
                        self.set_health(runtime, container_id, Health::Empty);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Stop => {
                        self.set_state(runtime, container_id, State::Stopping);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Die => {
                        self.set_state(runtime, container_id, State::Exited);
                        self.set_health(runtime, container_id, Health::Empty);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Pause => {
                        self.set_state(runtime, container_id, State::Paused);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Unpause => {
                        self.set_state(runtime, container_id, State::Running);
                    }
                    Action::Restart => {
                        self.set_state(runtime, container_id, State::Restarting);
                        self.set_health(runtime, container_id, Health::Empty);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Kill => {
                        self.set_state(runtime, container_id, State::Exited);
                        self.set_health(runtime, container_id, Health::Empty);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Remove => {
                        if let Some(containers) = self.containers.get_mut(runtime) {
                            containers.remove(container_id);
                        }
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Rename => {
                        if let Some(name) = &container_event.name
                            && let Some(container) = self
                                .containers
                                .get_mut(runtime)
                                .and_then(|containers| containers.get_mut(container_id))
                        {
                            container.name = name.clone();
                        }
                    }
                    Action::Oom => {
                        self.set_state(runtime, container_id, State::Exited);
                        self.set_health(runtime, container_id, Health::Empty);
                        self.clear_usage(runtime, container_id);
                    }
                    Action::Update => {}
                    Action::Health(health) => {
                        if health == Health::Healthy {
                            self.set_state(runtime, container_id, State::Running);
                        }
                        self.set_health(runtime, container_id, health);
                    }
                    Action::Unknown(_) => {}
                }
            }
            CollectorEvent::Stats { runtime, samples } => {
                let usage = self.usage.entry(runtime).or_default();

                for (id, sample) in samples {
                    usage.insert(id, sample);
                }

                rebuild = false;
            }
            CollectorEvent::RuntimeUp { runtime } => {
                self.runtimes.insert(runtime, RuntimeStatus::Up);
            }
            CollectorEvent::RuntimeDown { runtime, .. } => {
                self.runtimes.insert(runtime, RuntimeStatus::Down);
                self.usage.remove(&runtime);
            }
        }

        if rebuild {
            self.rebuild_visible();
        }

        Ok(())
    }

    fn clear_usage(&mut self, runtime: &Runtime, container_id: &str) {
        if let Some(usage) = self.usage.get_mut(runtime) {
            usage.remove(container_id);
        }
    }

    fn expire_usage(&mut self) {
        let now = Instant::now();

        for usage in self.usage.values_mut() {
            usage.retain(|_, sample| {
                now.saturating_duration_since(sample.sampled_at) <= STATS_STALE_AFTER
            });
        }

        self.usage.retain(|_, usage| !usage.is_empty());
    }

    pub fn usage(&self, runtime: Runtime, container_id: &str) -> Option<&ResourceUsage> {
        self.usage
            .get(&runtime)
            .and_then(|usage| usage.get(container_id))
    }

    fn set_state(&mut self, runtime: &Runtime, container_id: &str, state: State) {
        if let Ok(container) = self.container_mut(runtime, container_id) {
            container.state = state;
        }
    }

    fn set_health(&mut self, runtime: &Runtime, container_id: &str, health: Health) {
        if let Ok(container) = self.container_mut(runtime, container_id) {
            container.health = health;
        }
    }

    fn container_mut(
        &mut self,
        runtime: &Runtime,
        container_id: &str,
    ) -> Result<&mut Container, AppError> {
        self.containers
            .get_mut(runtime)
            .and_then(|containers| containers.get_mut(container_id))
            .ok_or_else(|| AppError::ContainerNotFound {
                runtime: *runtime,
                container_id: container_id.to_owned(),
            })
    }

    fn rebuild_visible(&mut self) {
        let selected = self.visible.get(self.selected).cloned();
        let filtering = self.is_filtered();

        let mut rows: Vec<(&str, Runtime, &str, &str)> = {
            let query = self.search.as_str();

            self.containers
                .iter()
                .flat_map(|(runtime, containers)| {
                    containers.values().map(move |c| {
                        (
                            c.name.as_str(),
                            *runtime,
                            c.id.as_str(),
                            c.compose_project.as_deref().unwrap_or(""),
                        )
                    })
                })
                .filter(|(name, _, _, _)| search::matches(query, name))
                .collect()
        };

        rows.sort_by(|a, b| a.3.cmp(b.3).then_with(|| a.0.cmp(b.0)));

        self.visible = rows
            .into_iter()
            .map(|(_, runtime, id, _)| (runtime, id.to_owned()))
            .collect();

        self.selected =
            match selected.and_then(|key| self.visible.iter().position(|row| *row == key)) {
                Some(position) => position,
                None if filtering => 0,
                None => self.selected.min(self.visible.len().saturating_sub(1)),
            };
    }

    pub fn container(&self, visible_pos: usize) -> (Runtime, &Container) {
        let (runtime, id) = self
            .visible
            .get(visible_pos)
            .or_else(|| self.visible.last())
            .expect("Visible list should not be empty");

        let container = self
            .containers
            .get(runtime)
            .expect("Runtime must exist in containers map") // Replaced unwrap()
            .get(id)
            .expect("Container ID must exist in runtime map");

        (*runtime, container)
    }

    pub fn container_rows(&self) -> Vec<(String, Vec<(Runtime, &Container)>)> {
        let mut projects: HashMap<String, Vec<(Runtime, &Container)>> = HashMap::new();

        for (runtime, id) in &self.visible {
            if let Some(container) = self
                .containers
                .get(runtime)
                .and_then(|containers| containers.get(id))
            {
                let project = container.compose_project.clone().unwrap_or_default();

                projects
                    .entry(project)
                    .or_default()
                    .push((*runtime, container));
            }
        }

        let mut result: Vec<(String, Vec<(Runtime, &Container)>)> = projects.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    pub fn runtimes(&self) -> Vec<(Runtime, RuntimeStatus)> {
        self.runtimes
            .iter()
            .map(|(runtime, status)| (*runtime, *status))
            .collect()
    }

    pub fn down_runtimes(&self) -> Vec<Runtime> {
        self.runtimes
            .iter()
            .filter(|(_, status)| **status == RuntimeStatus::Down)
            .map(|(runtime, _)| *runtime)
            .collect()
    }

    pub fn request_refresh(&mut self) {
        self.scheduled_sync = true;
    }

    pub fn begin_search(&mut self) {
        self.searching = true;
    }

    pub fn confirm_search(&mut self) {
        self.searching = false;
    }

    pub fn clear_search(&mut self) {
        self.searching = false;

        if self.search.is_empty() {
            return;
        }

        self.search.clear();
        self.rebuild_visible();
    }

    pub fn push_search_char(&mut self, character: char) {
        self.search.push(character);
        self.rebuild_visible();
    }

    pub fn pop_search_char(&mut self) {
        if self.search.pop().is_some() {
            self.rebuild_visible();
        }
    }

    pub fn is_searching(&self) -> bool {
        self.searching
    }

    pub fn is_filtered(&self) -> bool {
        !self.search.is_empty()
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    pub fn total_len(&self) -> usize {
        self.containers.values().map(HashMap::len).sum()
    }

    pub fn drain_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContainerEvent;

    fn container(runtime: Runtime, id: &str) -> Container {
        named_container(runtime, id, &format!("name-{id}"))
    }

    fn named_container(runtime: Runtime, id: &str, name: &str) -> Container {
        Container {
            id: id.to_owned(),
            name: name.to_owned(),
            image: "image:latest".to_owned(),
            runtime,
            state: State::Exited,
            health: Health::Healthy,
            compose_project: None,
        }
    }

    fn app_with(runtime: Runtime, containers: Vec<Container>) -> App {
        let mut app = App::new(vec![runtime]);
        app.apply_event(CollectorEvent::Snapshot {
            runtime,
            containers,
        })
        .unwrap();
        app
    }

    fn event(runtime: Runtime, id: &str, action: Action) -> CollectorEvent {
        CollectorEvent::Container(ContainerEvent {
            runtime,
            id: id.to_owned(),
            name: None,
            action,
        })
    }

    fn usage() -> ResourceUsage {
        ResourceUsage {
            cpu_percent: Some(12.5),
            memory_used: 100,
            memory_limit: 200,
            memory_percent: Some(50.0),
            network_rx_bytes: 10,
            network_tx_bytes: 20,
            network_rx_rate: Some(1),
            network_tx_rate: Some(2),
            block_read_bytes: 30,
            block_write_bytes: 40,
            block_read_rate: Some(3),
            block_write_rate: Some(4),
            pids: Some(5),
            sampled_at: Instant::now(),
        }
    }

    fn stats_event(runtime: Runtime, id: &str) -> CollectorEvent {
        CollectorEvent::Stats {
            runtime,
            samples: vec![(id.to_owned(), usage())],
        }
    }

    fn assert_container(app: &App, runtime: Runtime, id: &str, state: State, health: Health) {
        let container = &app.containers[&runtime][id];
        assert_eq!(container.state, state);
        assert_eq!(container.health, health);
    }

    #[test]
    fn new_marks_runtimes_down() {
        let app = App::new(vec![Runtime::Docker, Runtime::Podman]);

        assert_eq!(app.runtimes[&Runtime::Docker], RuntimeStatus::Down);
        assert_eq!(app.runtimes[&Runtime::Podman], RuntimeStatus::Down);
    }

    #[test]
    fn snapshot_indexes_containers_by_id() {
        let app = app_with(
            Runtime::Docker,
            vec![
                container(Runtime::Docker, "c1"),
                container(Runtime::Docker, "c2"),
            ],
        );

        let containers = &app.containers[&Runtime::Docker];
        assert_eq!(containers.len(), 2);
        assert_eq!(containers["c1"].name, "name-c1");
        assert_eq!(containers["c2"].name, "name-c2");
    }

    #[test]
    fn snapshot_replaces_existing_containers() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "old")]);

        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "new")],
        })
        .unwrap();

        let containers = &app.containers[&Runtime::Docker];
        assert!(!containers.contains_key("old"));
        assert!(containers.contains_key("new"));
    }

    #[test]
    fn snapshot_only_replaces_target_runtime() {
        let mut app = App::new(vec![Runtime::Docker, Runtime::Podman]);

        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![container(Runtime::Docker, "docker-1")],
        })
        .unwrap();
        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Podman,
            containers: vec![container(Runtime::Podman, "podman-1")],
        })
        .unwrap();

        assert!(app.containers[&Runtime::Docker].contains_key("docker-1"));
        assert!(app.containers[&Runtime::Podman].contains_key("podman-1"));
    }

    #[test]
    fn search_filters_visible_containers_by_fuzzy_name() {
        let mut app = app_with(
            Runtime::Docker,
            vec![
                named_container(Runtime::Docker, "c1", "postgres"),
                named_container(Runtime::Docker, "c2", "redis"),
                named_container(Runtime::Docker, "c3", "pgadmin"),
            ],
        );

        for character in "pgs".chars() {
            app.push_search_char(character);
        }

        assert!(app.is_filtered());
        assert_eq!(app.visible_len(), 1);
        assert_eq!(app.total_len(), 3);
        assert_eq!(app.container(0).1.name, "postgres");
    }

    #[test]
    fn search_preserves_selection_when_it_still_matches() {
        let mut app = app_with(
            Runtime::Docker,
            vec![
                named_container(Runtime::Docker, "c1", "api"),
                named_container(Runtime::Docker, "c2", "postgres"),
                named_container(Runtime::Docker, "c3", "web"),
            ],
        );
        app.selected = 1;

        app.push_search_char('e');

        assert_eq!(app.selected, 0);
        assert_eq!(app.container(app.selected).1.name, "postgres");
    }

    #[test]
    fn search_moves_selection_to_first_match_when_it_disappears() {
        let mut app = app_with(
            Runtime::Docker,
            vec![
                named_container(Runtime::Docker, "c1", "api"),
                named_container(Runtime::Docker, "c2", "postgres"),
                named_container(Runtime::Docker, "c3", "web"),
            ],
        );
        app.selected = 2;

        for character in "pos".chars() {
            app.push_search_char(character);
        }

        assert_eq!(app.selected, 0);
        assert_eq!(app.container(app.selected).1.name, "postgres");
    }

    #[test]
    fn clearing_search_restores_all_containers() {
        let mut app = app_with(
            Runtime::Docker,
            vec![
                named_container(Runtime::Docker, "c1", "api"),
                named_container(Runtime::Docker, "c2", "postgres"),
            ],
        );

        app.push_search_char('z');

        assert_eq!(app.visible_len(), 0);
        assert!(!app.is_searching());

        app.begin_search();
        app.clear_search();

        assert!(!app.is_filtered());
        assert!(!app.is_searching());
        assert_eq!(app.visible_len(), 2);
    }

    #[test]
    fn search_applies_to_new_events() {
        let mut app = app_with(
            Runtime::Docker,
            vec![named_container(Runtime::Docker, "c1", "postgres")],
        );

        for character in "pgs".chars() {
            app.push_search_char(character);
        }

        app.apply_event(CollectorEvent::Container(ContainerEvent {
            runtime: Runtime::Docker,
            id: "c2".to_owned(),
            name: Some("pgsidecar".to_owned()),
            action: Action::Create,
        }))
        .unwrap();
        app.apply_event(CollectorEvent::Container(ContainerEvent {
            runtime: Runtime::Docker,
            id: "c3".to_owned(),
            name: Some("redis".to_owned()),
            action: Action::Create,
        }))
        .unwrap();

        assert_eq!(app.visible_len(), 2);
    }

    #[test]
    fn backspacing_search_updates_the_filter() {
        let mut app = app_with(
            Runtime::Docker,
            vec![
                named_container(Runtime::Docker, "c1", "postgres"),
                named_container(Runtime::Docker, "c2", "redis"),
            ],
        );

        for character in "redisx".chars() {
            app.push_search_char(character);
        }
        assert_eq!(app.visible_len(), 0);

        app.pop_search_char();

        assert_eq!(app.search_query(), "redis");
        assert_eq!(app.visible_len(), 1);
        assert_eq!(app.container(0).1.name, "redis");
    }

    #[test]
    fn create_inserts_placeholder_container() {
        let mut app = App::new(vec![Runtime::Docker]);

        app.apply_event(CollectorEvent::Container(ContainerEvent {
            runtime: Runtime::Docker,
            id: "c1".to_owned(),
            name: Some("name-c1".to_owned()),
            action: Action::Create,
        }))
        .unwrap();

        assert_container(&app, Runtime::Docker, "c1", State::Created, Health::Empty);
        assert_eq!(app.containers[&Runtime::Docker]["c1"].name, "name-c1");
    }

    #[test]
    fn create_keeps_existing_container() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(CollectorEvent::Container(ContainerEvent {
            runtime: Runtime::Docker,
            id: "c1".to_owned(),
            name: Some("replacement".to_owned()),
            action: Action::Create,
        }))
        .unwrap();

        assert_container(&app, Runtime::Docker, "c1", State::Exited, Health::Healthy);
        assert_eq!(app.containers[&Runtime::Docker]["c1"].name, "name-c1");
    }

    #[test]
    fn lifecycle_actions_update_state_and_health() {
        let cases = [
            (Action::Start, State::Running, Health::Empty),
            (Action::Stop, State::Stopping, Health::Healthy),
            (Action::Die, State::Exited, Health::Empty),
            (Action::Pause, State::Paused, Health::Healthy),
            (Action::Unpause, State::Running, Health::Healthy),
            (Action::Restart, State::Restarting, Health::Empty),
            (Action::Oom, State::Exited, Health::Empty),
        ];

        for (action, state, health) in cases {
            let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

            app.apply_event(event(Runtime::Docker, "c1", action))
                .unwrap();

            assert_container(&app, Runtime::Docker, "c1", state, health);
        }
    }

    #[test]
    fn destroy_removes_container() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(Runtime::Docker, "c1", Action::Remove))
            .unwrap();

        assert!(!app.containers[&Runtime::Docker].contains_key("c1"));
    }

    #[test]
    fn health_action_updates_health_only() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(
            Runtime::Docker,
            "c1",
            Action::Health(Health::Unhealthy),
        ))
        .unwrap();

        assert_container(
            &app,
            Runtime::Docker,
            "c1",
            State::Exited,
            Health::Unhealthy,
        );
    }

    #[test]
    fn rename_updates_container_name() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(CollectorEvent::Container(ContainerEvent {
            runtime: Runtime::Docker,
            id: "c1".to_owned(),
            name: Some("renamed".to_owned()),
            action: Action::Rename,
        }))
        .unwrap();

        assert_eq!(app.containers[&Runtime::Docker]["c1"].name, "renamed");
    }

    #[test]
    fn rename_without_name_keeps_existing_name() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(Runtime::Docker, "c1", Action::Rename))
            .unwrap();

        assert_eq!(app.containers[&Runtime::Docker]["c1"].name, "name-c1");
    }

    #[test]
    fn unknown_action_leaves_container_unchanged() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(event(
            Runtime::Docker,
            "c1",
            Action::Unknown("exec_start".to_owned()),
        ))
        .unwrap();

        assert_container(&app, Runtime::Docker, "c1", State::Exited, Health::Healthy);
    }

    #[test]
    fn event_for_unknown_container_is_ignored() {
        let mut app = App::new(vec![Runtime::Docker]);

        let result = app.apply_event(event(Runtime::Docker, "missing", Action::Start));
        assert!(result.is_ok());
        assert!(!app.containers.contains_key(&Runtime::Docker));
    }

    #[test]
    fn stats_event_stores_usage() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(stats_event(Runtime::Docker, "c1")).unwrap();

        let usage = app.usage(Runtime::Docker, "c1").unwrap();
        assert_eq!(usage.cpu_percent, Some(12.5));
        assert_eq!(usage.memory_used, 100);
    }

    #[test]
    fn snapshot_prunes_usage_for_containers_that_are_no_longer_running() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);
        app.apply_event(stats_event(Runtime::Docker, "c1")).unwrap();
        app.apply_event(stats_event(Runtime::Docker, "gone"))
            .unwrap();

        let mut running = container(Runtime::Docker, "c1");
        running.state = State::Running;

        app.apply_event(CollectorEvent::Snapshot {
            runtime: Runtime::Docker,
            containers: vec![running],
        })
        .unwrap();

        assert!(app.usage(Runtime::Docker, "c1").is_some());
        assert!(app.usage(Runtime::Docker, "gone").is_none());
    }

    #[test]
    fn lifecycle_events_clear_usage() {
        for action in [
            Action::Start,
            Action::Stop,
            Action::Pause,
            Action::Die,
            Action::Kill,
            Action::Oom,
            Action::Remove,
        ] {
            let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);
            app.apply_event(stats_event(Runtime::Docker, "c1")).unwrap();

            app.apply_event(event(Runtime::Docker, "c1", action.clone()))
                .unwrap();

            assert!(
                app.usage(Runtime::Docker, "c1").is_none(),
                "usage should be cleared after {action:?}"
            );
        }
    }

    #[test]
    fn runtime_down_clears_usage() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);
        app.apply_event(stats_event(Runtime::Docker, "c1")).unwrap();

        app.apply_event(CollectorEvent::RuntimeDown {
            runtime: Runtime::Docker,
            reason: "boom".to_owned(),
        })
        .unwrap();

        assert!(app.usage(Runtime::Docker, "c1").is_none());
    }

    #[test]
    fn stale_usage_expires_on_tick() {
        let mut app = app_with(Runtime::Docker, vec![container(Runtime::Docker, "c1")]);

        app.apply_event(CollectorEvent::Stats {
            runtime: Runtime::Docker,
            samples: vec![(
                "c1".to_owned(),
                ResourceUsage {
                    sampled_at: Instant::now() - STATS_STALE_AFTER - Duration::from_secs(1),
                    ..usage()
                },
            )],
        })
        .unwrap();

        app.tick();

        assert!(app.usage(Runtime::Docker, "c1").is_none());
    }

    #[test]
    fn runtime_events_update_status() {
        let mut app = App::new(vec![Runtime::Docker]);

        app.apply_event(CollectorEvent::RuntimeUp {
            runtime: Runtime::Docker,
        })
        .unwrap();
        assert_eq!(app.runtimes[&Runtime::Docker], RuntimeStatus::Up);

        app.apply_event(CollectorEvent::RuntimeDown {
            runtime: Runtime::Docker,
            reason: "boom".to_owned(),
        })
        .unwrap();
        assert_eq!(app.runtimes[&Runtime::Docker], RuntimeStatus::Down);
    }

    #[test]
    fn down_runtimes_returns_only_down_runtimes() {
        let mut app = App::new(vec![Runtime::Docker, Runtime::Podman]);

        let mut down = app.down_runtimes();
        down.sort();
        assert_eq!(down, vec![Runtime::Docker, Runtime::Podman]);

        app.apply_event(CollectorEvent::RuntimeUp {
            runtime: Runtime::Docker,
        })
        .unwrap();

        assert_eq!(app.down_runtimes(), vec![Runtime::Podman]);

        app.apply_event(CollectorEvent::RuntimeUp {
            runtime: Runtime::Podman,
        })
        .unwrap();

        assert!(app.down_runtimes().is_empty());
    }
}
