use crate::model::Container;

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub input: String,
    pub applied: String,
    previous: String,
}

impl Filter {
    pub fn matches(&self, container: &Container) -> bool {
        if self.applied.is_empty() {
            return true;
        }

        let query = self.applied.to_ascii_lowercase();

        container.name.to_ascii_lowercase().contains(&query)
            || container.image.to_ascii_lowercase().contains(&query)
            || container.id.to_ascii_lowercase().starts_with(&query)
            || container.runtime.to_string().contains(&query)
    }

    pub fn is_active(&self) -> bool {
        !self.applied.is_empty()
    }

    pub fn begin(&mut self) {
        self.previous = self.applied.clone();
        self.input = self.applied.clone();
    }

    pub fn edit(&mut self) {
        self.applied = self.input.clone();
    }

    pub fn commit(&mut self) {
        self.previous.clear();
    }

    pub fn cancel(&mut self) {
        self.input = self.previous.clone();
        self.applied = self.previous.clone();
        self.previous.clear();
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.applied.clear();
        self.previous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Health, Runtime, State};

    fn container(name: &str) -> Container {
        Container {
            id: "05ae208348f4".to_owned(),
            name: name.to_owned(),
            image: "docker.io/library/postgres:18".to_owned(),
            runtime: Runtime::Docker,
            state: State::Running,
            health: Health::Healthy,
        }
    }

    #[test]
    fn empty_filter_matches_everything() {
        assert!(Filter::default().matches(&container("postgres")));
    }

    #[test]
    fn matches_name_image_id_prefix_and_runtime() {
        let mut filter = Filter::default();

        for query in ["post", "POST", "postgres:18", "05ae", "docker"] {
            filter.applied = query.to_owned();
            assert!(filter.matches(&container("postgres")), "query: {query}");
        }

        filter.applied = "05ae2".to_owned();
        assert!(filter.matches(&container("postgres")));

        filter.applied = "ae20".to_owned();
        assert!(!filter.matches(&container("postgres")));
    }

    #[test]
    fn cancel_restores_the_applied_query() {
        let mut filter = Filter {
            applied: "post".to_owned(),
            ..Filter::default()
        };
        filter.begin();
        filter.input.push_str("gres");
        filter.edit();
        assert_eq!(filter.applied, "postgres");

        filter.cancel();
        assert_eq!(filter.applied, "post");
    }

    #[test]
    fn clear_resets_every_field() {
        let mut filter = Filter {
            applied: "post".to_owned(),
            ..Filter::default()
        };
        filter.begin();
        filter.clear();

        assert!(filter.input.is_empty());
        assert!(filter.applied.is_empty());
        assert!(!filter.is_active());
    }
}
