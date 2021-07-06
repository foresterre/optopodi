use async_trait::async_trait;
use chrono::Duration;
use stable_eyre::eyre;
use tokio::sync::mpsc::Sender;

use super::{util, Graphql, Producer};
use crate::report::util::GitHubOrganization;

#[derive(Debug)]
pub struct ListReposForOrg {
    graphql: Graphql,
    org_repos: Vec<GitHubOrganization>,
    number_of_days: i64,
}

impl ListReposForOrg {
    pub fn new(graphql: Graphql, org_repos: Vec<GitHubOrganization>, number_of_days: i64) -> Self {
        ListReposForOrg {
            graphql,
            org_repos,
            number_of_days,
        }
    }
}

#[async_trait]
impl Producer for ListReposForOrg {
    fn column_names(&self) -> Vec<String> {
        vec![String::from("Repository Name"), String::from("# of PRs")]
    }

    async fn producer_task(mut self, tx: Sender<Vec<String>>) -> Result<(), eyre::Error> {
        for org in &self.org_repos {
            for repo in org.repository_names() {
                let count_prs = util::count_pull_requests(
                    &mut self.graphql,
                    org.organisation_name(),
                    &repo,
                    Duration::days(self.number_of_days),
                )
                .await?;

                tx.send(vec![repo.to_owned(), count_prs.to_string()])
                    .await?;
            }
        }

        Ok(())
    }
}
