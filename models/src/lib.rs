use config_loader::ConfigType;
use config_loader_derive::ConfigType;
use uuid::Uuid;

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
