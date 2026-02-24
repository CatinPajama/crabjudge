pub mod email;
pub mod exec;
pub mod utils;

pub use exec::*;

use std::{collections::HashMap};
use urlencoding::encode;

#[derive(serde::Deserialize, PartialEq, Debug)]

pub struct RuntimeConfigs(pub HashMap<String, RuntimeConfig>);

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct RuntimeConfig {
    pub run: String,
    pub compile: Option<String>,
    pub image: String,
    pub timeout: u8,
    pub memory: i64,
    pub env: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct WorkerTask {
    pub code: String,
    pub problem_id: i64,
    pub user_id: i64,
    pub submission_id: i64,
}

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct ApiConfig {
    pub port: u16,
    pub host: String,
    pub base_url: String,
}

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct DatabaseConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub dbname: String,
    pub port: u16,
    pub admin_username: String,
}

impl DatabaseConfig {
    pub fn url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.dbname
        )
    }
}

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
}

impl RedisConfig {
    pub fn url(&self) -> String {
        format!("redis://{}:{}", self.host, self.port)
    }
}

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct RabbitMQConfig {
    pub host: String,
    pub port: u16,
    pub vhost: String,
}

impl RabbitMQConfig {
    pub fn url(&self) -> String {
        let encoded_vhost = encode(&self.vhost);
        format!("amqp://{}:{}/{}", self.host, self.port, encoded_vhost)
    }
}
