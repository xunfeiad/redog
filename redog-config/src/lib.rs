pub mod types;

pub use types::*;

use redog_core::Error;

/// Load and validate config from a YAML file
pub fn load_config(path: &str) -> Result<Config, Error> {
    let content =
        std::fs::read_to_string(path).map_err(|e| Error::Config(format!("read config: {}", e)))?;
    parse_config(&content)
}

/// Parse config from YAML string
pub fn parse_config(content: &str) -> Result<Config, Error> {
    let config: Config =
        serde_yaml::from_str(content).map_err(|e| Error::Config(format!("parse yaml: {}", e)))?;
    config.validate()?;
    Ok(config)
}
