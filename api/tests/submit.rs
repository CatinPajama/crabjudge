mod utils;
use futures_util::StreamExt;
use lapin::{
    ConnectionProperties,
    options::{BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
};
use models::{Settings, WorkerTask};
use std::collections::HashMap;
use utils::spawn_app;

#[tokio::test]
async fn send_code() {
    let app = spawn_app().await;

    let settings = Settings::get_configuration().unwrap();
    let conn =
        lapin::Connection::connect(&settings.rabbitmq.url(), ConnectionProperties::default())
            .await
            .expect("Unable to connect to rabbitmq");

    let channel = conn.create_channel().await.unwrap();

    channel
        .exchange_declare(
            "test",
            lapin::ExchangeKind::Direct,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    channel
        .queue_declare(
            "test",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    channel
        .queue_bind(
            "test",
            "test",
            "",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

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

    let expected_output = serde_json::json!({
        "code": "print('hi)",
        "env": "test",
    });

    let response = client
        .post(format!("http://127.0.0.1:{}/1/submit", app.port))
        .json(&expected_output)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let mut consume = channel
        .basic_consume(
            "test",
            "test",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let delivery = consume.next().await.unwrap().unwrap();

    let output: WorkerTask = serde_json::from_slice(&delivery.data).unwrap();

    assert_eq!(output.code, *expected_output.get("code").unwrap());
}
