use crate::{BOT_CONFIG, GITHUB_HTTP_CLIENT, SLACK_HTTP_CLIENT};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};

use rocket::serde::json::{serde_json::json, Json, Value};
use rocket::serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[allow(clippy::upper_case_acronyms)]
type URI = String;

// GraphQL queries
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github.graphql",
    query_path = "src/queries/single_select_field_name.graphql",
    response_derives = "Debug"
)]
struct ProjectFieldStatus;
use project_field_status::ProjectFieldStatusNode::ProjectV2SingleSelectField;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github.graphql",
    query_path = "src/queries/item.graphql",
    response_derives = "Debug"
)]
struct Item;
use item::{
    ItemNode::ProjectV2Item,
    ItemNodeOnProjectV2ItemContent::{Issue, PullRequest},
    ItemNodeOnProjectV2ItemFieldValuesNodes::ProjectV2ItemFieldSingleSelectValue,
};

// Receivers
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct GitHubProjectsPayload<'r> {
    action: Cow<'r, str>,
    sender: Sender<'r>,
    changes: Option<ChangesField<'r>>,
    projects_v2_item: ProjectItem<'r>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct Sender<'r> {
    login: Cow<'r, str>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ChangesField<'r> {
    field_node_id: Option<Cow<'r, str>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ProjectItem<'r> {
    node_id: Cow<'r, str>,
    project_node_id: Cow<'r, str>,
}

#[post("/github_projects_activity", format = "json", data = "<message>")]
async fn github_projects_activity(message: Json<GitHubProjectsPayload<'_>>) -> Option<Value> {
    // Inputs
    // Maybe consider avoiding .to_string() invocation and instead have it String by default from
    // struct level.
    let action = message.action.to_string();
    let node_id = message.projects_v2_item.node_id.to_string();
    let sender_login = message.sender.login.to_string();

    // Get GitHub http client
    let github_http_client = GITHUB_HTTP_CLIENT.get()?;

    // Get config
    let bot_config = BOT_CONFIG.get()?;

    // If the `action` was "edited", check whether any change was made in a field called "Status".
    // If "edited" but the modified field was not "Status", then exit.
    if action == "edited" {
        if let Some(value) = &message.changes.as_ref().unwrap().field_node_id {
            // Variables to pass into graphql
            let variables = project_field_status::Variables {
                input_field_node_id: value.to_string(),
            };

            let project_item_response = post_graphql::<ProjectFieldStatus, _>(
                github_http_client,
                &bot_config.github.graphql_api_endpoint,
                variables,
            )
            .await
            .unwrap()
            .data?
            .node?;

            if let ProjectV2SingleSelectField(ref on_match) = project_item_response {
                if on_match.name != "Status" {
                    return None;
                }
            }
        }
    }

    // Get the item and status that it changed to
    let variables = item::Variables {
        input_node_id: node_id.to_string(),
    };

    let item_response = post_graphql::<Item, _>(
        github_http_client,
        &bot_config.github.graphql_api_endpoint,
        variables,
    )
    .await
    .unwrap()
    .data?
    .node?;

    if let ProjectV2Item(item) = item_response {
        // TODO: We could explicitly use ID instead of the "title" (i.e. "name")
        let project_name = Cow::from(item.project.title.to_lowercase());
        let project_name = html_escape::decode_html_entities(&project_name);

        let (item_url, item_title) = match item.content? {
            Issue(object) => (object.url, object.title),
            PullRequest(object) => (object.url, object.title),
            _ => unreachable!(),
        };
        let item_title = html_escape::decode_html_entities(&item_title);

        // Set the "Status" for the item in the Project
        let updated_project_item_status = item
            .field_values
            .nodes
            .into_iter()
            .flatten()
            .find_map(|node| {
                if let Some(ProjectV2ItemFieldSingleSelectValue(field_value)) = node {
                    field_value.name
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "No status".to_owned());

        // IDE team specific tasks
        if project_name.contains("ide") {
            let project_node_id = message.projects_v2_item.project_node_id.to_string();
        }

        let message = match action.as_str() {
            "edited" => {
                format!("<{item_url}|{item_title}> set to *{updated_project_item_status}*")
            }
            _ => format!("<{item_url}|{item_title}> added to project"),
        } + &format!(" by <https://githbub.com/{sender_login}|{sender_login}>");

        // Post to slack based on team name
        for team in &bot_config.slack.teams {
            if project_name.contains(&team.name) {
                // Determine which channel to post
                let channel_id = if action == "edited" {
                    &team.edited_channel_id
                } else {
                    &team.created_channel_id
                };

                let payload = json!({
                  "channel": channel_id.as_ref().unwrap(),
                  "blocks": [
                    {
                      "type": "section",
                      "text": {
                        "type": "mrkdwn",
                        "text": message,
                      }
                    }
                  ]
                });

                SLACK_HTTP_CLIENT
                    .get()?
                    .post(bot_config.slack.api_endpoint.clone() + "/chat.postMessage")
                    .json(&payload)
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap();
            }
        }
    }

    Some(json!({ "status": "ok", "sender_login": sender_login }))
}

#[catch(404)]
fn not_found() -> Value {
    json!({
        "status": "error",
        "reason": "Resource was not found."
    })
}

pub fn stage() -> rocket::fairing::AdHoc {
    rocket::fairing::AdHoc::on_ignite("JSON", |rocket| async {
        rocket
            .mount("/json", routes![github_projects_activity])
            .register("/json", catchers![not_found])
    })
}
