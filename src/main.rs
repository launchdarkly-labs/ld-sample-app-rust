use std::env;
use std::sync::Arc;

use handlebars::Handlebars;
use launchdarkly_server_sdk::{Client, ConfigBuilder, Context, ContextBuilder};
use serde::Serialize;
use serde_json::json;
use warp::Filter;

struct WithTemplate<T: Serialize> {
    name: &'static str,
    value: T,
}

fn render<T>(template: WithTemplate<T>, hbs: Arc<Handlebars<'_>>) -> impl warp::Reply
where
    T: Serialize,
{
    let render = hbs
        .render(template.name, &template.value)
        .unwrap_or_else(|err| err.to_string());
    warp::reply::html(render)
}

fn context_str() -> Context {
    ContextBuilder::new("018ee873-7b09-7f26-b296-0358b2ff1c87")
        .kind("device")
        .name("Linux")
        .build()
        .expect("Failed to build context")
}

#[tokio::main]
async fn main() {
    // Connect to LaunchDarkly
    let sdkkey = env::var("LD_SDK_KEY").expect("'LD_SDK_KEY' key not set");
    let config = ConfigBuilder::new(&sdkkey)
        .build()
        .expect("Config failed to build.");
    let client = Arc::new(Client::build(config).expect("Client failed to build."));
    client.start_with_default_executor();
    if !client.initialized_async().await {
        panic!("Client failed to initialize");
    }

    // Setup HTML template
    let mut hb = Handlebars::new();
    hb.register_template_file("index", "./templates/index.html")
        .unwrap();
    let hb = Arc::new(hb);
    let handlebars = move |with_template| render(with_template, hb.clone());

    // Set route to template
    let client_filter = warp::any().map(move || client.clone());
    let route = warp::get()
        .and(warp::path::end())
        .and(client_filter.clone())
        .map(move |client: Arc<Client>| {
            let flagvalue = client
                .bool_variation(&context_str(), "test-flag", false)
                .to_string();
            WithTemplate {
                name: "index",
                value: json!({"flagvalue": flagvalue}),
            }
        })
        .map(handlebars);

    let (_addr, svc) =
        warp::serve(route).bind_with_graceful_shutdown(([0, 0, 0, 0], 8000), async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Couldn't bind interrupt signal");
        });
    svc.await;

    println!("Service closed successfully");
}
