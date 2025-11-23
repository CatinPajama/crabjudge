use std::{
    env,
    path::{Path, PathBuf},
};

use config::ConfigError;

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct ExecEnv {
    image: String,
}

#[derive(serde::Deserialize, PartialEq, Debug)]
pub struct Settings {
    images: Vec<ExecEnv>,
}
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
pub fn get_configuration(base_path: &Path) -> Result<Settings, ConfigError> {
    // let config_root = Path::new("../configuration/");

    let env: Environment = env::var("APP_ENV")
        .unwrap_or_else(|_| "local".to_string())
        .try_into()
        .unwrap();

    // let base = base_path.join("base.yaml");
    let extra = base_path.join(format!("{}.yaml", env));
    let settings = config::Config::builder()
        //        .add_source(config::File::from(base))
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

    #[test]
    fn get_images() -> anyhow::Result<()> {
        let base_path = Path::new("./");
        let file_path = base_path.join(Path::new("./local.yaml"));
        let mut file = File::create(&file_path).unwrap();
        let actual_setting = Settings {
            images: vec![
                ExecEnv {
                    image: "python:3.12-slim".to_string(),
                },
                ExecEnv {
                    image: "cpp".to_string(),
                },
            ],
        };

        if let Err(e) = file.write_all(
            b"images:
            - image : python:3.12-slim
            - image : cpp",
        ) {
            std::fs::remove_file(file_path).unwrap();
            panic!("{}", e);
        }
        match get_configuration(base_path) {
            Err(e) => {
                std::fs::remove_file(file_path).unwrap();
                panic!("{}", e);
            }
            Ok(settings) => {
                assert_eq!(actual_setting, settings);
            }
        }
        std::fs::remove_file(file_path).unwrap();
        Ok(())
    }
}
