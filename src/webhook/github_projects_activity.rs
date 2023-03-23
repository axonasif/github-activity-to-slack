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

use self::project::ProjectNode::ProjectV2;
use self::project::ProjectNodeOnProjectV2Field::ProjectV2IterationField;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github.graphql",
    query_path = "src/queries/project.graphql",
    response_derives = "Debug"
)]
struct Project;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github.graphql",
    query_path = "src/queries/add_item_week.graphql",
    response_derives = "Debug"
)]
struct AddItemWeek;

// Receivers
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct GitHubProjectsPayload<'r> {
    action: Cow<'r, str>,
    sender: Sender<'r>,
    changes: Option<ChangesField<'r>>,
    projects_v2_item: ProjectStruct<'r>,
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
struct ProjectStruct<'r> {
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
    let project_node_id = message.projects_v2_item.project_node_id.to_string();
    let sender_login = message.sender.login.to_string();

    // Get GitHub http client
    let github_http_client = GITHUB_HTTP_CLIENT.get()?;

    // Get config
    let bot_config = BOT_CONFIG.get()?;

    // If the `action` was "edited", check whether any change was made in a field called "Status".
    // If "edited" but the modified field was not "Status", then exit.
    if action == "edited" && let Some(value) = &message.changes.as_ref()?.field_node_id {
        // Variables to pass into graphql
        let variables = project_field_status::Variables {
            input_field_node_id: value.to_string(),
        };

        let project_response = post_graphql::<ProjectFieldStatus, _>(
            github_http_client,
            &bot_config.github.graphql_api_endpoint,
            variables,
        )
        .await
        .ok()?
        .data?
        .node?;

        if let ProjectV2SingleSelectField(ref on_match) = project_response {
            if on_match.name != "Status" {
                // Do not continue anymore
                return None;
            }
        }
    }

    // Get the item and status that it changed to
    let variables = item::Variables {
        input_node_id: node_id.clone(),
    };

    let item_response = post_graphql::<Item, _>(
        github_http_client,
        &bot_config.github.graphql_api_endpoint,
        variables,
    )
    .await
    .ok()?
    .data?
    .node?;

    if let ProjectV2Item(item) = item_response {
        let project_name = Cow::from(item.project.title.to_lowercase());
        let project_name = html_escape::decode_html_entities(&project_name);
        let item_contents = item.content?;

        let (item_url, item_title) = match &item_contents {
            Issue(object) => (&object.url, &object.title),
            PullRequest(object) => (&object.url, &object.title),
            _ => unreachable!(),
        };
        let item_title = html_escape::decode_html_entities(&item_title);

        // Set the "Status" for the item in the Project
        let updated_project_status = item
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

        let slack_message = match action.as_str() {
            "edited" => {
                format!("<{item_url}|{item_title}> set to *{updated_project_status}*")
            }
            _ => format!("<{item_url}|{item_title}> added to project"),
        } + &format!(" by <https://githbub.com/{sender_login}|{sender_login}>");

        // Post to slack based on team name
        for team in &bot_config.automations.github_projects.to_slack_teams {
            // Check if the Config.toml value matches with an explicit project_node_id
            // or contains the provided name.
            if project_node_id == team.github_project_name_or_id
            || project_name.contains(&team.github_project_name_or_id) {
                // Determine which channel to post
                let channel_id = if action == "edited" {
                    &team.slack_edited_channel_id
                } else {
                    &team.slack_created_channel_id
                };

                let payload = json!({
                  "channel": channel_id.as_ref()?,
                  "blocks": [
                    {
                      "type": "section",
                      "text": {
                        "type": "mrkdwn",
                        "text": slack_message,
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
                    .ok()?
                    .text()
                    .await
                    .ok()?;
            }
        }

        // A team specific task
        let project_name_or_id = &bot_config.automations.github_projects.iteration_increment_project_name_or_id;

        if let Some(project) = project_name_or_id && (project_node_id == *project || project_name.contains(project))
        && let Issue(content) = &item_contents && let Some(labels) = &content.labels {
            let is_epic = labels
                .nodes
                .as_ref()
                .into_iter()
                .flatten()
                .filter_map(|node| node.as_ref())
                .any(|label| label.name == "type: epic");


            // If "Week" field is not set and the item is not labled with "type: epic"
            if item.field_value_by_name.is_none() && !is_epic {
                // Get the current possible iteration IDs from Project config/settings
                let variables = project::Variables {
                    input_project_node_id: project_node_id.clone(),
                };

                let project_response = post_graphql::<Project, _>(
                    github_http_client,
                    &bot_config.github.graphql_api_endpoint,
                    variables,
                )
                .await
                .ok()?
                .data?
                .node?;

                // Set "Week" iteration value
                // This sets the next (upcoming) week if exists or the current one otherwise.
                if let ProjectV2(project) = project_response
                && let Some(ProjectV2IterationField(field)) = project.field
                && let Some(iteration) = field.configuration.iterations.get(1).or_else(|| field.configuration.iterations.get(0)) {
                    
                    // Get the current possible iteration IDs from Project config/settings
                    let variables = add_item_week::Variables {
                       input_node_id: node_id, 
                        input_project_node_id: project_node_id,
                        input_field_id: field.id,
                        input_iterations_id: iteration.id.clone(),
                    };

                    post_graphql::<AddItemWeek, _>(
                        github_http_client,
                        &bot_config.github.graphql_api_endpoint,
                        variables,
                    )
                    .await
                    .ok()?;
                }
            }
        }
    }

    Some(json!({ "status": "ok" }))
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
