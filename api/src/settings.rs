use std::path::Path;

use models::{
    ApiConfig, DatabaseConfig, RabbitMQConfig, RedisConfig, RuntimeConfigs,
    email::EmailClientConfig, utils::get_configuration,
};

#[derive(serde::Deserialize)]
pub struct ApiSettings {
    pub application: ApiConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub rabbitmq: RabbitMQConfig,
    pub runtimeconfigs: RuntimeConfigs,
    pub email_client: EmailClientConfig,
}

impl ApiSettings {
    pub fn get_configuration() -> Result<ApiSettings, config::ConfigError> {
        let base = Path::new("./configuration");
        get_configuration::<ApiSettings>(base)
    }
}
