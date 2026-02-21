use std::path::Path;

use models::{DatabaseConfig, RabbitMQConfig, RuntimeConfig, utils::get_configuration};

#[derive(serde::Deserialize)]
pub struct WorkerSettings {
    pub database: DatabaseConfig,
    pub rabbitmq: RabbitMQConfig,
    pub runtimeconfig: RuntimeConfig,
}

impl WorkerSettings {
    pub fn get_configuration() -> Result<WorkerSettings, config::ConfigError> {
        let base = Path::new("./configuration");
        return Ok(get_configuration::<WorkerSettings>(base)?);
    }
}
