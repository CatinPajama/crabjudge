use std::path::Path;

use config_loader::{ConfigType, get_configuration};
use config_loader_derive::ConfigType;
use urlencoding::encode;

#[derive(serde::Deserialize, PartialEq, Debug, ConfigType)]
pub struct RuntimeConfigs {
    pub runtimeconfigs: Vec<RuntimeConfig>,
}

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct RuntimeConfig {
    pub version: Option<String>,
    pub command: String,
    pub image: String,
    pub timeout: usize,
    pub memory: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct WorkerTask {
    pub code: String,
    pub problem_id: i64,
    pub user_id: i64,
}

#[derive(serde::Deserialize, PartialEq, Debug, ConfigType)]
pub struct ApiConfig {
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize, PartialEq, Debug, ConfigType)]
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

#[derive(serde::Deserialize, PartialEq, Debug, ConfigType)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
}

impl RedisConfig {
    pub fn url(&self) -> String {
        format!("redis://{}:{}", self.host, self.port)
    }
}

#[derive(serde::Deserialize, PartialEq, Debug, ConfigType)]
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
pub struct Settings {
    pub application: ApiConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub rabbitmq: RabbitMQConfig,
}

impl Settings {
    pub fn get_configuration() -> Result<Settings, config::ConfigError> {
        let base = Path::new("../configuration");
        Ok(Settings {
            application: get_configuration::<ApiConfig>(base)?,
            database: get_configuration::<DatabaseConfig>(base)?,
            redis: get_configuration::<RedisConfig>(base)?,
            rabbitmq: get_configuration::<RabbitMQConfig>(base)?,
        })
    }
}
