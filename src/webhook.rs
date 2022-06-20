use regex::Regex;
use rocket::serde::json::serde_json::json;
use rocket::serde::json::{Json, Value};
use rocket::serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct Message<'r> {
    action: Cow<'r, str>,
    projects_v2_item: ProjectItem<'r>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ProjectItem<'r> {
    node_id: Cow<'r, str>,
}

#[post("/github_projects_assignment", format = "json", data = "<message>")]
async fn github_projects_assignment(message: Json<Message<'_>>) -> Value {
    let node_id = message
        .projects_v2_item
        .node_id
        .to_string()
        .replace("PVTI_", "PNI_");

    let query = format!(
        r#"{{"query": "query {{ node(id: \"{}\") {{ ... on ProjectNextItem {{ content {{ ... on Issue {{ url }} }} }} }} }}"}}"#,
        node_id
    );
    let resp = reqwest::Client::new()
        .post("https://api.github.com/graphql")
        .header("User-Agent", "axonasif")
        .header(
            "Authorization",
            "bearer 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",
        )
        .body(query)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let re = Regex::new(r#""url":"(?P<url>.*?)""#)
        .unwrap()
        .captures(resp.as_str())
        .unwrap();
    println!("{}", re.name("url").unwrap().as_str());

    json!({ "status": "ok" })
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct FrontApp<'r> {
    node_id: Cow<'r, str>,
}

#[post("/frontapp", format = "json", data = "<message>")]
async fn frontapp(message: Json<FrontApp<'_>>) -> Value {
    let lol = "";

    json!("")
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
            .mount("/json", routes![github_projects_assignment])
            .register("/json", catchers![not_found])
    })
}
