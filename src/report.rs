use std::path::Path;
use std::sync::Arc;
use std::{fs::File, path::PathBuf};

use fehler::throws;
use serde::Deserialize;
use stable_eyre::eyre;
use stable_eyre::eyre::{Error, WrapErr};

use crate::metrics::Consumer;
use crate::metrics::{self, Graphql};
use std::collections::HashMap as Map;
use std::convert::TryFrom;
use std::str::FromStr;

mod high_contributor;
mod repo_info;
mod repo_participant;
mod top_crates;
pub(crate) mod util;

pub struct Report {
    /// Directory where to store the data.
    data_dir: PathBuf,

    /// If true, load the saved graphql queries from disk.
    replay_graphql: bool,
}

#[derive(Debug, Deserialize)]
struct ReportConfig {
    project: Vec<ProjectConfig>,
    high_contributor: HighContributorConfig,
    data_source: DataSourceConfig,
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "IntermediateProjectConfig")]
struct ProjectConfig(Vec<ProjectConfigValue>);

impl TryFrom<IntermediateProjectConfig> for ProjectConfig {
    type Error = eyre::Error;

    fn try_from(value: IntermediateProjectConfig) -> Result<Self, Self::Error> {
        let result = value
            .0
            .into_iter()
            .map(|(k, v)| v.map_by_pair(&k))
            .collect::<eyre::Result<Vec<_>>>()?;

        Ok(ProjectConfig(result))
    }
}

#[derive(Debug, Deserialize)]
struct IntermediateProjectConfig(Map<String, ProjectConfigValue>);

impl ReportConfig {
    fn github_projects(&self) -> Vec<&Github> {
        self.project
            .iter()
            .filter_map(|project| match project {
                ProjectConfigValue::Github(project) => Some(project),
            })
            .collect()
    }

    fn github_projects_mut(&mut self) -> Vec<&mut Github> {
        self.project
            .iter_mut()
            .filter_map(|project| match project {
                ProjectConfigValue::Github(project) => Some(project),
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct ReportData {
    repo_participants: repo_participant::RepoParticipants,
    repo_infos: repo_info::RepoInfos,
    top_crates: Vec<top_crates::TopCrateInfo>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ProjectConfigValue {
    #[serde(rename = "github")]
    Github(Github),
}

impl ProjectConfigValue {
    // The array of project tables is deserialized to a vec of (hash)maps.
    // The key of a key-value pair in the project table, is not necessarily identifiable as the type
    // of the value. Since we require that the key and value in the
    // key-value pair (k, v) match  `e.g. the key github should always have be the `Github` variant of
    // the `ProjectConfigValue`), and since we don't recognize this at deserialization time, we have to
    // verify it in hindsight.
    // FIXME: It would be more elegant to solve this during deserialization.
    fn map_by_pair(self, identifier: &str) -> eyre::Result<Self> {
        match (identifier, &self) {
            ("github", Self::Github(_)) => Ok(self),
            (field, _) => eyre::bail!(
                "Invalid field '{}' found for 'project' in report.toml",
                field
            ),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Github {
    org: String,
    repos: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct DataSourceConfig {
    number_of_days: i64,
}

#[derive(Deserialize, Debug)]
struct HighContributorConfig {
    /// Number of categories one must be "high" in
    /// to be considered a "high contributor".
    high_reviewer_min_percentage: u64,

    high_reviewer_min_prs: u64,

    reviewer_saturation_threshold: u64,

    author_saturation_threshold: u64,

    high_participant_min_percentage: u64,

    high_participant_min_prs: u64,

    high_author_min_percentage: u64,

    high_author_min_prs: u64,

    /// Number of categories one must be "high" in
    /// to be considered a "high contributor".
    high_contributor_categories_threshold: u64,
}

impl Report {
    pub fn new(data_dir: PathBuf, replay_graphql: bool) -> Self {
        Report {
            data_dir,
            replay_graphql,
        }
    }

    #[throws]
    pub async fn run(mut self) {
        // Load the report configuration from the data directory.
        let config = Arc::new(self.load_config().await?);

        tokio::fs::create_dir_all(self.graphql_dir()).await?;
        tokio::fs::create_dir_all(self.input_dir()).await?;
        tokio::fs::create_dir_all(self.output_dir()).await?;

        let data = Arc::new(ReportData {
            top_crates: self.top_crates(&config).await?,
            repo_participants: self.repo_participants(&config).await?,
            repo_infos: self.repo_infos(&config).await?,
        });

        tokio::task::spawn_blocking(move || -> eyre::Result<()> {
            self.write_top_crates(&config, &data)?;
            self.write_high_contributors(&config, &data)?;
            Ok(())
        })
        .await??;
    }

    #[throws]
    async fn load_config(&mut self) -> ReportConfig {
        let report_config_file = self.data_dir.join("report.toml");
        let report_config_bytes = tokio::fs::read_to_string(&report_config_file).await?;
        let mut config: ReportConfig = toml::from_str(&report_config_bytes)
            .wrap_err_with(|| format!("Unable to parse '{}'", report_config_file.display()))?;

        for project in config.github_projects_mut() {
            if project.repos.is_empty() {
                let graphql = &mut self.graphql("all-repos");
                project.repos = metrics::all_repos(graphql, &project.org).await?;
            }
        }

        config
    }

    fn graphql(&self, dir_name: &str) -> Graphql {
        let graphql_dir = self.graphql_dir().join(dir_name);
        Graphql::new(graphql_dir, self.replay_graphql)
    }

    fn graphql_dir(&self) -> PathBuf {
        self.data_dir.join("graphql")
    }

    fn input_dir(&self) -> PathBuf {
        self.data_dir.join("inputs")
    }

    fn output_dir(&self) -> PathBuf {
        self.data_dir.join("output")
    }

    #[throws]
    async fn produce_input(&self, path: &Path, producer: impl metrics::Producer + Send + 'static) {
        let (column_names, mut rx) = metrics::run_producer(producer);
        let f = File::create(path)?;
        metrics::Print::new(f)
            .consume(&mut rx, column_names)
            .await?;
    }
}
