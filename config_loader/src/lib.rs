use config_loader_derive::ConfigType;
use std::{
    env,
    path::{Path, PathBuf},
};

use config::ConfigError;

pub trait ConfigType {
    fn get_config_name() -> String;
}

pub fn get_configuration<T: ConfigType + serde::de::DeserializeOwned + PartialEq>(
    base_path: &Path,
) -> Result<T, ConfigError> {
    let env = env::var("APP_ENV").unwrap_or_else(|_| "local".to_string());

    let env_dir = base_path.join(&env);
    let extra = env_dir.join(format!("{}.yaml", T::get_config_name()));
    let settings = config::Config::builder()
        .add_source(config::File::from(extra))
        .build()?;

    settings.try_deserialize()
}

#[cfg(test)]
mod test {
    use std::{
        fs::File,
        io::{self, Write},
        path::{Path, PathBuf},
    };

    use super::*;
    #[derive(serde::Deserialize, PartialEq, Debug, ConfigType)]
    struct RuntimeConfigs {
        runtimeconfigs: Vec<RuntimeConfig>,
    }

    #[derive(serde::Deserialize, PartialEq, Debug)]
    struct RuntimeConfig {
        version: Option<String>,
        command: String,
        image: String,
        timeout: usize,
        memory: usize,
    }

    #[test]
    fn get_images() {
        std::fs::create_dir("local").unwrap();
        let base_path = Path::new("./");
        let file_path = base_path.join(Path::new("./local/runtimeconfigs.yaml"));
        let mut file = File::create(&file_path).unwrap();
        let actual_setting = vec![RuntimeConfig {
            image: "python:3.12-slim".to_string(),
            version: None,
            command: "python".to_string(),
            memory: 2048,
            timeout: 2,
        }];

        if let Err(e) = file.write_all(
            b"
            runtimeconfigs:
                -   image: python:3.12-slim
                    command: python
                    timeout: 2
                    memory: 2048
            ",
        ) {
            std::fs::remove_dir_all("local").unwrap();
            panic!("{}", e);
        }
        match get_configuration::<RuntimeConfigs>(base_path) {
            Err(e) => {
                std::fs::remove_dir_all("local").unwrap();
                panic!("{}", e);
            }
            Ok(settings) => {
                assert_eq!(actual_setting, settings.runtimeconfigs);
            }
        }
        std::fs::remove_dir_all("local").unwrap();
    }
}
