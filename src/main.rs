use clap::{AppSettings, Clap};
use fehler::throws;
use stable_eyre::eyre::Error;
use std::path::PathBuf;

mod metrics;
mod report;
mod serde;
mod token;
mod util;

use crate::report::Report;

#[derive(Clap, Debug, PartialEq)]
#[clap(setting = AppSettings::ColoredHelp)]
#[clap(name = "optopodi")]
struct OctoCli {
    /// Load the saved results of grapql queries from disk (if they are present).
    #[clap(long)]
    replay_graphql: bool,

    /// the sub-command to run
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Clap, Debug, PartialEq)]
enum Cmd {
    Report { directory: String },
}

#[throws]
#[tokio::main]
async fn main() {
    stable_eyre::install()?;
    env_logger::init();

    let token = token::github_token()?;

    // initialize static octocrab API -- call `octocrab::instance()` anywhere to retrieve instance
    octocrab::initialise(octocrab::Octocrab::builder().personal_token(token))?;

    let cli = OctoCli::parse();

    match cli.cmd {
        Cmd::Report { directory } => {
            Report::new(PathBuf::from(directory), cli.replay_graphql)
                .run()
                .await?;
        }
    }
}
