#[macro_use]
extern crate rocket;

mod webhook;

#[launch]
fn rocket() -> _ {
    rocket::build().attach(webhook::stage())
}
