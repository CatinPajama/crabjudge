use std::env;
use std::path::Path;

use config::{Config, ConfigError, File, FileSourceFile, Source};

enum Environment {
    Local,
    Production,
}

impl TryFrom<String> for Environment {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            _ => Err("No such environemnt"),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Local => write!(f, "local"),
            Environment::Production => write!(f, "production"),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub database: DatabaseSettings,
    pub redis_uri: String,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub user: String,
    pub password: String,
    pub host: String,
    pub dbname: String,
    pub port: u16,
}

impl DatabaseSettings {
    pub fn url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.dbname
        )
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    let config_root = Path::new("../configuration/");

    let env: Environment = env::var("APP_ENV")
        .unwrap_or_else(|_| "local".to_string())
        .try_into()
        .unwrap();

    let base = config_root.join("base.yaml");
    let extra = config_root.join(format!("{}.yaml", env));
    let settings = Config::builder()
        .add_source(File::from(base))
        .add_source(File::from(extra))
        .build()?;

    settings.try_deserialize()
}

#[cfg(test)]
pub mod tests {
    use crate::configuration::get_configuration;

    #[test]
    fn test_reading_configuration() {
        let settings = get_configuration().unwrap();
        assert_eq!(settings.application.port, 8000);
    }
}
