use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub github: GithubConfig,
    pub slack: SlackConfig,
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
    pub teams: Vec<Team>,
    pub user_agent: String,
}

#[derive(Debug, Deserialize)]
pub struct Team {
    pub name: String,
    pub created_channel_id: Option<String>,
    pub edited_channel_id: Option<String>,
    // pub github_project_id: u64,
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
