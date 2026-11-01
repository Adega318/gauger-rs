use crate::model::Runtime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorCommand {
    RefreshAll,
}

impl CollectorCommand {
    pub(super) fn applies_to(&self, _runtime: Runtime) -> bool {
        matches!(self, Self::RefreshAll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_all_applies_to_every_runtime() {
        assert!(CollectorCommand::RefreshAll.applies_to(Runtime::Docker));
        assert!(CollectorCommand::RefreshAll.applies_to(Runtime::Podman));
    }
}
