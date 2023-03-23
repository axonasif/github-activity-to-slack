use serde::Deserialize;
use std::path::Path;

// Top level
#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub github: GithubConfig,
    pub slack: SlackConfig,
    pub automations: AutomationsConfig,
}

#[derive(Debug, Deserialize)]
pub struct GithubConfig {
    pub api_token: String,
    pub graphql_api_endpoint: String,
    pub user_agent: String,
}

#[derive(Debug, Deserialize)]
pub struct SlackConfig {
    pub api_token: String,
    pub api_endpoint: String,
    pub bot_name: String,
    pub bot_avatar: String,
    pub user_agent: String,
}

#[derive(Debug, Deserialize)]
pub struct AutomationsConfig {
    pub github_projects: GitHubProjects,
}

#[derive(Debug, Deserialize)]
pub struct GitHubProjects {
    pub iteration_increment_project_name_or_id: Option<String>,
    pub to_slack_teams: Vec<GitHubSlackTeams>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubSlackTeams {
    pub github_project_name_or_id: String,
    pub slack_created_channel_id: Option<String>,
    pub slack_edited_channel_id: Option<String>,
}

pub fn read(toml_path: &str) -> BotConfig {
    // Check
    if Path::new(&toml_path).try_exists().is_err() {
        panic!("{toml_path} does not exist");
    }

    // Read the TOML file into a var
    let contents = std::fs::read_to_string(toml_path)
        .expect("Couldn't read a Config.toml file in the current directory or provided path");
    // Parse the TOML string into a `Config` object
    let config: BotConfig = toml::from_str(&contents).expect("Failed to parse TOML");

    // Return
    config
}
