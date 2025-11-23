use std::collections::HashMap;

use api::run;

use crate::utils::spawn_app;
mod utils;
#[tokio::test]
async fn login_successful() {
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let mut form_data = HashMap::new();
    form_data.insert("username", "alice");
    form_data.insert("password", "secret123");
    let response = client
        .post(format!("http://127.0.0.1:{}/signup", app.port))
        .form(&form_data)
        .send()
        .await
        .expect("unable to signup");

    assert_eq!(response.status(), 200);
    /*
        let response = client
            .post(format!("http://127.0.0.1:{}/login", app.port))
            .basic_auth("alice", Some("secret123"))
            .send()
            .await
            .expect("Unable to send reqwest");

        assert_eq!(response.status(), 200);
    */
}

#[tokio::test]
async fn login_should_fail() {
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let mut form_data = HashMap::new();
    form_data.insert("username", "alice");
    form_data.insert("password", "secret123");
    let response = client
        .post(format!("http://127.0.0.1:{}/signup", app.port))
        .form(&form_data)
        .send()
        .await
        .expect("unable to signup");

    assert_eq!(response.status(), 200);

    let response = client
        .post(format!("http://127.0.0.1:{}/login", app.port))
        .basic_auth("alice", Some("randomshit"))
        .send()
        .await
        .expect("Unable to send reqwest");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn send_code() {
    let app = spawn_app().await;

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let mut form_data = HashMap::new();
    form_data.insert("username", "alice");
    form_data.insert("password", "secret123");
    let response = client
        .post(format!("http://127.0.0.1:{}/signup", app.port))
        .form(&form_data)
        .send()
        .await
        .expect("unable to signup");

    assert_eq!(response.status(), 200);

    let response = client
        .post(format!("http://127.0.0.1:{}/login", app.port))
        .basic_auth("alice", Some("secret123"))
        .send()
        .await
        .expect("Unable to send reqwest");

    assert_eq!(response.status(), 200);

    let response = client
        .post(format!("http://127.0.0.1:{}/1200/submit", app.port))
        .json(&serde_json::json!(
                {
                "code" : "print('hi')",
                "env" : "python:3.12-slim"
                }
        ))
        .send()
        .await
        .unwrap();
    println!("{:?}", response);
}
