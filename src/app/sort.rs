use std::cmp::Ordering;

use crate::model::Container;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    Runtime,
    Name,
    State,
    Health,
}

impl SortKey {
    pub fn next(self) -> Self {
        match self {
            Self::Runtime => Self::Name,
            Self::Name => Self::State,
            Self::State => Self::Health,
            Self::Health => Self::Runtime,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Name => "name",
            Self::State => "state",
            Self::Health => "health",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sort {
    pub key: SortKey,
    pub descending: bool,
}

impl Sort {
    pub fn compare(&self, a: &Container, b: &Container) -> Ordering {
        let ordering = match self.key {
            SortKey::Runtime => a.runtime.cmp(&b.runtime).then_with(|| name_cmp(a, b)),
            SortKey::Name => name_cmp(a, b).then_with(|| a.runtime.cmp(&b.runtime)),
            SortKey::State => a.state.cmp(&b.state).then_with(|| name_cmp(a, b)),
            SortKey::Health => a.health.cmp(&b.health).then_with(|| name_cmp(a, b)),
        };

        if self.descending {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

fn name_cmp(a: &Container, b: &Container) -> Ordering {
    a.name
        .to_ascii_lowercase()
        .cmp(&b.name.to_ascii_lowercase())
        .then_with(|| a.id.cmp(&b.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Health, Runtime, State};

    fn container(name: &str, runtime: Runtime, state: State, health: Health) -> Container {
        Container {
            id: name.to_owned(),
            name: name.to_owned(),
            image: "image".to_owned(),
            runtime,
            state,
            health,
        }
    }

    fn names(sort: &Sort, containers: &[Container]) -> Vec<String> {
        let mut containers = containers.to_vec();
        containers.sort_by(|a, b| sort.compare(a, b));
        containers
            .into_iter()
            .map(|container| container.name)
            .collect()
    }

    #[test]
    fn defaults_to_runtime_then_name() {
        let containers = [
            container("zeta", Runtime::Podman, State::Running, Health::Healthy),
            container("beta", Runtime::Docker, State::Running, Health::Healthy),
            container("alpha", Runtime::Docker, State::Running, Health::Healthy),
        ];

        assert_eq!(
            names(&Sort::default(), &containers),
            vec!["alpha", "beta", "zeta"]
        );
    }

    #[test]
    fn sorts_by_name_case_insensitively() {
        let containers = [
            container("Zulu", Runtime::Docker, State::Running, Health::Healthy),
            container("alpha", Runtime::Docker, State::Running, Health::Healthy),
        ];
        let sort = Sort {
            key: SortKey::Name,
            descending: false,
        };

        assert_eq!(names(&sort, &containers), vec!["alpha", "Zulu"]);
    }

    #[test]
    fn reverses_direction() {
        let containers = [
            container("alpha", Runtime::Docker, State::Running, Health::Healthy),
            container("beta", Runtime::Docker, State::Running, Health::Healthy),
        ];
        let sort = Sort {
            key: SortKey::Name,
            descending: true,
        };

        assert_eq!(names(&sort, &containers), vec!["beta", "alpha"]);
    }

    #[test]
    fn cycles_every_sort_key() {
        let mut key = SortKey::Runtime;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(key);
            key = key.next();
        }

        assert_eq!(
            seen,
            vec![
                SortKey::Runtime,
                SortKey::Name,
                SortKey::State,
                SortKey::Health
            ]
        );
        assert_eq!(key, SortKey::Runtime);
    }
}
