#[macro_use]
extern crate rocket;

use once_cell::sync::OnceCell;
use reqwest::header::{self, HeaderValue};
use reqwest::Client;

mod config;
mod webhook;
use webhook::*;

// HTTP client
static GITHUB_HTTP_CLIENT: OnceCell<Client> = OnceCell::new();
static SLACK_HTTP_CLIENT: OnceCell<Client> = OnceCell::new();
static BOT_CONFIG: OnceCell<config::BotConfig> = OnceCell::new();

#[launch]
fn rocket() -> _ {
    // Fetch config and store
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Config.toml".to_owned());
    BOT_CONFIG.set(config::read(&config_path)).unwrap();
    let config = BOT_CONFIG.get().unwrap();
    println!(">> Loaded config from {config_path}");

    // GitHub API client
    let github_graphql_api_builder = Client::builder()
        .default_headers(
            [
                (
                    header::USER_AGENT,
                    HeaderValue::from_static(&config.github.user_agent),
                ),
                (
                    header::AUTHORIZATION,
                    format!("Bearer {}", config.github.api_token)
                        .parse()
                        .expect("Can't parse token"),
                ),
                (
                    header::ACCEPT,
                    HeaderValue::from_static("application/vnd.github+json"),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .build()
        .expect("Can't build http client");

    GITHUB_HTTP_CLIENT
        .set(github_graphql_api_builder)
        .expect("Failed to cache GitHub HTTP client");

    // Slack API client
    let slack_api_builder = Client::builder()
        .default_headers(
            [
                (
                    header::USER_AGENT,
                    HeaderValue::from_static(&config.slack.user_agent),
                ),
                (
                    header::AUTHORIZATION,
                    format!("Bearer {}", config.slack.api_token)
                        .parse()
                        .expect("Can't parse token"),
                ),
                (header::ACCEPT, HeaderValue::from_static("application/json")),
            ]
            .into_iter()
            .collect(),
        )
        .build()
        .expect("Can't build http client");

    SLACK_HTTP_CLIENT
        .set(slack_api_builder)
        .expect("Failed to cache Slack HTTP client");

    rocket::build().attach(github_projects_activity::stage())
}
