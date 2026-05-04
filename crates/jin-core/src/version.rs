use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRecord {
    pub name: String,
    pub source_ref: String,
    pub artifact_path: String,
    pub health: HealthStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRegistry {
    stable: VersionRecord,
    candidate: Option<VersionRecord>,
    previous_stable: Option<VersionRecord>,
}

impl VersionRegistry {
    pub fn new(stable: VersionRecord) -> Self {
        Self {
            stable,
            candidate: None,
            previous_stable: None,
        }
    }

    pub fn stable(&self) -> &VersionRecord {
        &self.stable
    }

    pub fn previous_stable(&self) -> Option<&VersionRecord> {
        self.previous_stable.as_ref()
    }

    pub fn set_candidate(&mut self, candidate: VersionRecord) {
        self.candidate = Some(candidate);
    }

    pub fn promote_candidate(&mut self) -> Result<(), VersionError> {
        let candidate = self.candidate.take().ok_or(VersionError::NoCandidate)?;
        if candidate.health != HealthStatus::Healthy {
            self.candidate = Some(candidate);
            return Err(VersionError::CandidateUnhealthy);
        }

        let previous = std::mem::replace(&mut self.stable, candidate);
        self.previous_stable = Some(previous);
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<String, VersionError> {
        let previous = self
            .previous_stable
            .take()
            .ok_or(VersionError::NoPreviousStable)?;
        let artifact_path = previous.artifact_path.clone();
        let current = std::mem::replace(&mut self.stable, previous);
        self.candidate = Some(current);
        Ok(artifact_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    NoCandidate,
    CandidateUnhealthy,
    NoPreviousStable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_registry_promotes_healthy_candidate_and_rolls_back_to_previous_stable() {
        let mut registry = VersionRegistry::new(VersionRecord {
            name: "stable-a".to_string(),
            source_ref: "commit-a".to_string(),
            artifact_path: "/opt/jin/stable-a".to_string(),
            health: HealthStatus::Healthy,
        });

        registry.set_candidate(VersionRecord {
            name: "candidate-b".to_string(),
            source_ref: "commit-b".to_string(),
            artifact_path: "/opt/jin/candidate-b".to_string(),
            health: HealthStatus::Healthy,
        });

        registry
            .promote_candidate()
            .expect("healthy candidate should promote");
        assert_eq!(registry.stable().name, "candidate-b");
        assert_eq!(
            registry.previous_stable().expect("previous stable").name,
            "stable-a"
        );

        let artifact = registry
            .rollback()
            .expect("rollback should return artifact");
        assert_eq!(artifact, "/opt/jin/stable-a");
        assert_eq!(registry.stable().name, "stable-a");
    }
}
