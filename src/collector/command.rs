use crate::model::Runtime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorCommand {
    Refresh(Runtime),
    RefreshAll,
}

impl CollectorCommand {
    pub(super) fn applies_to(&self, runtime: Runtime) -> bool {
        match self {
            Self::Refresh(target) => *target == runtime,
            Self::RefreshAll => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_command_targets_one_runtime() {
        assert!(CollectorCommand::Refresh(Runtime::Docker).applies_to(Runtime::Docker));
        assert!(!CollectorCommand::Refresh(Runtime::Docker).applies_to(Runtime::Podman));
    }

    #[test]
    fn refresh_all_applies_to_every_runtime() {
        assert!(CollectorCommand::RefreshAll.applies_to(Runtime::Docker));
        assert!(CollectorCommand::RefreshAll.applies_to(Runtime::Podman));
    }
}
