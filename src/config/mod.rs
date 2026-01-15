use anyhow::Context;
use config::{Config, FileFormat};
use serde::Deserialize;
use std::sync::LazyLock;

static CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| AppConfig::load().expect("Failed to initialize config"));
    
// #[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {

    pub start_page: i32,
    pub end_page: i32,
    pub output_dir: String,

    pub catalogue_base_url: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        Config::builder()
            .add_source(
                config::File::with_name("application")
                    .format(FileFormat::Yaml)
                    .required(true),
            )
            .add_source(
                config::Environment::with_prefix("APP")
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(","),
            )
            .build()
            .with_context(|| anyhow::anyhow!("Failed to load config"))?
            .try_deserialize()
            .with_context(|| anyhow::anyhow!("Failed to deserialize config"))
    }
}

pub fn get() -> &'static AppConfig {
    &CONFIG
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_load_config() {
        let config = AppConfig::load().expect("Failed to load config");
        println!("{:#?}", config);
    }
}
