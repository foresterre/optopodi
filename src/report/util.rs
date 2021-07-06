use crate::report::Github;

/// The repositories as owned by a (GitHub) organisation.
#[derive(Debug)]
pub struct GitHubOrganization {
    organisation_name: String,
    repository_names: Vec<String>,
}

impl GitHubOrganization {
    pub fn new<'collect>(
        org_name: impl ToString,
        repo_names: impl IntoIterator<Item = &'collect String>,
    ) -> Self {
        Self {
            organisation_name: org_name.to_string(),
            repository_names: repo_names.into_iter().cloned().collect(),
        }
    }

    pub fn organisation_name(&self) -> &str {
        &self.organisation_name
    }

    pub fn repository_names(&self) -> &[String] {
        &self.repository_names
    }
}

pub fn organisation_repositories(projects: &[&Github]) -> Vec<GitHubOrganization> {
    projects
        .iter()
        .map(|project| GitHubOrganization::new(&project.org, &project.repos))
        .collect()
}
