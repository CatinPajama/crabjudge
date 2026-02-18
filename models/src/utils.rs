use std::path::Path;

use config::ConfigError;

pub fn get_configuration<T: serde::de::DeserializeOwned>(
    base_path: &Path,
) -> Result<T, ConfigError> {
    let env = "local".to_string();
    let extra = base_path.join(format!("{}.yaml", env));
    let settings = config::Config::builder()
        .add_source(
            config::Environment::with_prefix("CRABJUDGE")
                .prefix_separator("_")
                .separator("__"),
        )
        .add_source(config::File::from(extra).required(false))
        .build()?;
    settings.try_deserialize()
}
