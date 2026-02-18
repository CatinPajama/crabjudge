use std::path::Path;

use models::{DatabaseConfig, RabbitMQConfig, RuntimeConfigs, utils::get_configuration};

#[derive(serde::Deserialize)]
pub struct WorkerSettings {
    pub database: DatabaseConfig,
    pub rabbitmq: RabbitMQConfig,
    pub runtimeconfigs: RuntimeConfigs,
}

impl WorkerSettings {
    pub fn get_configuration() -> Result<WorkerSettings, config::ConfigError> {
        let base = Path::new("./configuration");
        return Ok(get_configuration::<WorkerSettings>(base)?);
    }
}
