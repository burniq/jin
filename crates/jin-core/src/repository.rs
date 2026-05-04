use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRegistration {
    name: String,
    root: PathBuf,
}

impl ProjectRegistration {
    pub fn new(name: impl Into<String>, root: PathBuf) -> Result<Self, RepositoryError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(RepositoryError::BlankProjectName);
        }

        Ok(Self { name, root })
    }
}

#[derive(Debug, Default)]
pub struct LocalProjectRegistry {
    projects: HashMap<String, ProjectRegistration>,
}

impl LocalProjectRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, project: ProjectRegistration) -> Result<(), RepositoryError> {
        let root = canonicalize_existing(&project.root)?;
        self.projects.insert(
            project.name.clone(),
            ProjectRegistration {
                name: project.name,
                root,
            },
        );
        Ok(())
    }

    pub fn resolve_workspace(
        &self,
        project_name: &str,
        requested_path: PathBuf,
    ) -> Result<PathBuf, RepositoryError> {
        let project = self
            .projects
            .get(project_name)
            .ok_or(RepositoryError::UnknownProject)?;
        let workspace = canonicalize_allow_missing(&requested_path)?;

        if workspace.starts_with(&project.root) {
            Ok(workspace)
        } else {
            Err(RepositoryError::WorkspaceOutsideRoot)
        }
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, RepositoryError> {
    path.canonicalize()
        .map_err(|_| RepositoryError::PathNotFound)
}

fn canonicalize_allow_missing(path: &Path) -> Result<PathBuf, RepositoryError> {
    let mut current = path;
    let mut missing = Vec::new();

    while !current.exists() {
        let name = current.file_name().ok_or(RepositoryError::PathNotFound)?;
        missing.push(name.to_os_string());
        current = current.parent().ok_or(RepositoryError::PathNotFound)?;
    }

    let mut resolved = current
        .canonicalize()
        .map_err(|_| RepositoryError::PathNotFound)?;
    for component in missing.iter().rev() {
        resolved.push(component);
    }

    Ok(resolved)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    BlankProjectName,
    UnknownProject,
    PathNotFound,
    WorkspaceOutsideRoot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_registry_rejects_workspaces_outside_registered_roots() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("jin-repository-test-{unique}"));
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        fs::create_dir_all(&allowed).expect("create allowed root");
        fs::create_dir_all(&outside).expect("create outside root");

        let mut registry = LocalProjectRegistry::new();
        registry
            .register(ProjectRegistration::new("jin", allowed.clone()).expect("valid project"))
            .expect("register project");

        let canonical_allowed = allowed.canonicalize().expect("canonical allowed root");
        let resolved = registry
            .resolve_workspace("jin", allowed.join("crates"))
            .expect("workspace under root should resolve");
        assert!(resolved.starts_with(&canonical_allowed));

        let rejected = registry.resolve_workspace("jin", outside);
        assert_eq!(rejected, Err(RepositoryError::WorkspaceOutsideRoot));

        fs::remove_dir_all(base).expect("clean temp dirs");
    }
}
