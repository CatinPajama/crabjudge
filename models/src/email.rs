use std::path::Path;

use config_loader::{ConfigType, get_configuration};
use config_loader_derive::ConfigType;
use reqwest::Client;
use validator::ValidateEmail;

use crate::EmailClientConfig;
#[derive(Debug)]
pub struct SubscriberEmail(String);

impl SubscriberEmail {
    pub fn parse(s: String) -> Result<SubscriberEmail, String> {
        if s.validate_email() {
            Ok(Self(s))
        } else {
            Err(format!("{} is not a valid subscriber email.", s))
        }
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SubscriberEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: String,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail, authorization_token: String) -> Self {
        Self {
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            base_url,
            sender,
            authorization_token,
        }
    }
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/api/send", self.base_url);
        let request_body = SendEmailRequest {
            from: SendEmailSender {
                email: self.sender.as_ref(),
                name: "CrabJudge",
            },
            to: [SendEmailReceiver {
                email: recipient.as_ref(),
            }],
            subject: subject,
            html: html_content,
            text: text_content,
        };
        let res = self
            .http_client
            .post(&url)
            .bearer_auth(&self.authorization_token)
            .json(&request_body)
            .send()
            .await?;
        res.error_for_status()?;
        Ok(())
    }
}
#[derive(serde::Serialize)]
struct SendEmailRequest<'a> {
    from: SendEmailSender<'a>,
    to: [SendEmailReceiver<'a>; 1],
    subject: &'a str,
    html: &'a str,
    text: &'a str,
}

#[derive(serde::Serialize)]
struct SendEmailSender<'a> {
    email: &'a str,
    name: &'a str,
}

#[derive(serde::Serialize)]
struct SendEmailReceiver<'a> {
    email: &'a str,
}

#[tokio::test]
async fn test_email() {
    let email_client_config =
        get_configuration::<EmailClientConfig>(Path::new("../configuration")).unwrap();
    let client = EmailClient::new(
        email_client_config.base_url.clone(),
        email_client_config.sender().unwrap(),
        email_client_config.authorization_token,
    );
    client
        .send_email(
            SubscriberEmail::parse("grb.khtry@gmail.com".to_string()).unwrap(),
            "test",
            "<h1>Hello</h1>",
            "hello",
        )
        .await
        .unwrap();
}
